//! The sync engine: pull Canvas into the local store.
//!
//! Design goals that the earlier client missed:
//!
//! * **Per-course isolation** — one failing course does not abort the rest; the
//!   error is collected and reported.
//! * **Upsert, never duplicate** — everything keys on Canvas ids.
//! * **Navigate the whole list** — the client's pagination makes this complete.
//! * **Local reads stay instant** — the UI reads from SQLite; the network is
//!   only touched here, in the background.

use chrono::Utc;
use yunee_canvas::{CanvasClient, types as cv};
use yunee_store as st;
use yunee_store::Store;

use crate::{html, state::Connection};

/// What one sync accomplished.
pub struct SyncReport {
    pub requested: bool,
    pub counts: st::SyncCounts,
    pub errors: Vec<String>,
    /// Unread announcements first seen since the previous sync — the raw
    /// material for a desktop notification.
    pub new_unread: Vec<(String, String)>,
}

impl SyncReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Pull everything for the configured connection into `store`.
pub async fn sync_all(store: &Store, connection: &Connection) -> SyncReport {
    sync_with_policy(store, connection, true).await
}

pub async fn sync_with_policy(store: &Store, connection: &Connection, force: bool) -> SyncReport {
    let mut report = SyncReport {
        requested: false,
        counts: Default::default(),
        errors: Vec::new(),
        new_unread: Vec::new(),
    };

    let known_announcements: std::collections::HashSet<String> =
        match store.list_all_announcements(i64::MAX) {
            Ok(rows) => rows.into_iter().map(|a| a.canvas_id).collect(),
            Err(e) => {
                report
                    .errors
                    .push(format!("could not read cached announcements: {e}"));
                return report;
            }
        };
    let now = Utc::now().to_rfc3339();
    if let Err(e) = store.record_sync("last_attempt", &now) {
        report
            .errors
            .push(format!("could not record sync attempt: {e}"));
        return report;
    }
    if let Err(e) = store.record_sync("last_result", "Sync interrupted before completion") {
        report.errors.push(e.to_string());
        return report;
    }

    let client = match connection.client() {
        Ok(client) => client,
        Err(e) => {
            report
                .errors
                .push(format!("could not build the client: {e}"));
            finish_sync(store, &now, &mut report);
            return report;
        }
    };

    let course_key = format!("{}|courses", client.base_url());
    if force || !fresh(store, &course_key, 5 * 60) {
        report.requested = true;
        let courses = match client.list_courses().await {
            Ok(courses) => courses,
            Err(e) => {
                report.errors.push(friendly(&e, "courses"));
                finish_sync(store, &now, &mut report);
                return report;
            }
        };
        let rows: Vec<_> = courses.iter().map(|c| map_course(c, &now)).collect();
        if let Err(e) = store
            .apply_snapshot(st::Snapshot::Courses(&rows))
            .and_then(|_| store.record_sync(&course_key, &now))
        {
            report.errors.push(format!("could not store courses: {e}"));
            finish_sync(store, &now, &mut report);
            return report;
        }
        report.counts.courses = rows.len();
    }
    let courses = match store.list_courses() {
        Ok(courses) => courses,
        Err(e) => {
            report.errors.push(e.to_string());
            finish_sync(store, &now, &mut report);
            return report;
        }
    };
    for course in &courses {
        if let Err(e) = sync_course(store, &client, course, &now, &mut report, force).await {
            report.errors.push(format!("{}: {e}", course.name));
        }
    }

    // Finish the durable side.
    finish_sync(store, &now, &mut report);

    // Announcements first observed in this run, including after a partial sync.
    if let Ok(all) = store.list_all_announcements(50) {
        for announcement in all {
            if !announcement.is_unread() {
                continue;
            }
            if !known_announcements.contains(&announcement.canvas_id) {
                let course = course_name_for(store, announcement.course_id);
                report.new_unread.push((course, announcement.title.clone()));
            }
        }
    }

    report
}

/// Freshness timestamps describe complete successful lists, never attempts.
fn fresh(store: &Store, key: &str, seconds: i64) -> bool {
    store
        .last_sync(key)
        .ok()
        .flatten()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(&v).ok())
        .map(|at| {
            let age = Utc::now().signed_duration_since(at);
            age >= chrono::Duration::zero() && age < chrono::Duration::seconds(seconds)
        })
        .unwrap_or(false)
}

pub fn files_fresh(store: &Store, connection: &Connection, course: &str) -> bool {
    connection.client().ok().is_some_and(|client| {
        let key = format!("{}|course:{course}|files", client.base_url());
        fresh(store, &key, 300) || fresh(store, &format!("{key}|missing"), 1800)
    })
}

/// Only a complete successful run advances the successful-sync timestamp.
fn finish_sync(store: &Store, now: &str, report: &mut SyncReport) {
    if report.requested {
        if let Err(e) = store.rebuild_search() {
            report.errors.push(format!("could not update search: {e}"));
        }
    }
    let outcome = if !report.requested && report.ok() {
        store.record_sync("last_result", "Cached data is fresh")
    } else {
        store.record_sync_outcome(now, &report.errors)
    };
    if let Err(e) = outcome {
        report
            .errors
            .push(format!("could not record sync result: {e}"));
    }
}

async fn sync_course(
    store: &Store,
    client: &CanvasClient,
    course: &st::Course,
    now: &str,
    report: &mut SyncReport,
    force: bool,
) -> Result<(), String> {
    let local_id = course.id;

    // Assignments (with the caller's submission state).
    let key = format!(
        "{}|course:{}|assignments",
        client.base_url(),
        course.canvas_id
    );
    if force || (!fresh(store, &key, 300) && !fresh(store, &format!("{key}|missing"), 1800)) {
        report.requested = true;
        let errors_before = report.errors.len();
        match client.list_assignments(&course.canvas_id).await {
            Ok(items) => {
                let rows: Vec<_> = items
                    .iter()
                    .map(|a| map_assignment(local_id, a, now))
                    .collect();
                store
                    .apply_snapshot(st::Snapshot::Assignments(local_id, &rows))
                    .map_err(|e| e.to_string())?;
                report.counts.assignments += rows.len();
                if report.errors.len() == errors_before {
                    store.record_sync(&key, now).map_err(|e| e.to_string())?;
                    store
                        .record_sync(&format!("{key}|missing"), "")
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(e) => return Err(friendly(&e, "assignments")),
        }
    }

    // Announcements.
    let key = format!(
        "{}|course:{}|announcements",
        client.base_url(),
        course.canvas_id
    );
    if force || (!fresh(store, &key, 300) && !fresh(store, &format!("{key}|missing"), 1800)) {
        report.requested = true;
        let errors_before = report.errors.len();
        match client.list_announcements(&course.canvas_id).await {
            Ok(topics) => {
                let rows: Vec<_> = topics
                    .iter()
                    .map(|a| map_announcement(local_id, a, now))
                    .collect();
                store
                    .apply_snapshot(st::Snapshot::Announcements(local_id, &rows))
                    .map_err(|e| e.to_string())?;
                report.counts.announcements += rows.len();
                if report.errors.len() == errors_before {
                    store.record_sync(&key, now).map_err(|e| e.to_string())?;
                    store
                        .record_sync(&format!("{key}|missing"), "")
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(e) => return Err(friendly(&e, "announcements")),
        }
    }

    // Modules + their items.
    let key = format!("{}|course:{}|modules", client.base_url(), course.canvas_id);
    if force || (!fresh(store, &key, 1800) && !fresh(store, &format!("{key}|missing"), 1800)) {
        report.requested = true;
        let errors_before = report.errors.len();
        match client.list_modules(&course.canvas_id).await {
            Ok(modules) => {
                let rows: Vec<_> = modules
                    .iter()
                    .map(|m| map_module(local_id, m, now))
                    .collect();
                store
                    .apply_snapshot(st::Snapshot::Modules(local_id, &rows))
                    .map_err(|e| e.to_string())?;
                report.counts.modules += rows.len();
                let stored = store.list_modules(local_id).map_err(|e| e.to_string())?;
                for module in &modules {
                    let module_local = stored
                        .iter()
                        .find(|m| m.canvas_id == module.id)
                        .ok_or_else(|| "module missing from local snapshot".to_string())?
                        .id;
                    // An absent/short inline list is not an authoritative empty list.
                    let items = if module.items_count == Some(module.items.len() as i64) {
                        module.items.clone()
                    } else {
                        match client
                            .list_module_items(&course.canvas_id, &module.id)
                            .await
                        {
                            Ok(items) => items,
                            Err(e) => {
                                report.errors.push(format!(
                                    "{}: {}",
                                    course.name,
                                    friendly(&e, "module items")
                                ));
                                continue;
                            }
                        }
                    };
                    let rows: Vec<_> = items
                        .iter()
                        .map(|i| map_module_item(local_id, module_local, i, now))
                        .collect();
                    store
                        .apply_snapshot(st::Snapshot::ModuleItems(module_local, &rows))
                        .map_err(|e| e.to_string())?;
                }
                if report.errors.len() == errors_before {
                    store.record_sync(&key, now).map_err(|e| e.to_string())?;
                    store
                        .record_sync(&format!("{key}|missing"), "")
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(e) if e.is_not_found() => {
                store
                    .record_sync(&format!("{key}|missing"), now)
                    .map_err(|e| e.to_string())?;
            } // older Canvas: skip quietly
            Err(e) => return Err(friendly(&e, "modules")),
        }
    }

    // Wiki pages. The index gives titles and slugs; each page's HTML body comes
    // from the show endpoint, fetched only when it is missing or changed, so a
    // re-sync costs almost nothing.
    let key = format!("{}|course:{}|pages", client.base_url(), course.canvas_id);
    if force || (!fresh(store, &key, 1800) && !fresh(store, &format!("{key}|missing"), 1800)) {
        report.requested = true;
        let errors_before = report.errors.len();
        match client.list_pages(&course.canvas_id).await {
            Ok(pages) => {
                let mut rows = Vec::new();
                for page in &pages {
                    let key = page
                        .page_id
                        .clone()
                        .or_else(|| page.url.clone())
                        .unwrap_or_default();
                    let stored = store.get_page(local_id, &key).map_err(|e| e.to_string())?;
                    let revision = page.updated_at.clone().unwrap_or_default();
                    let mut body_failed = false;
                    let body = match &stored {
                        Some(s) if s.has_body() && s.updated_at == revision => s.body.clone(),
                        _ => {
                            let lookup = page.url.clone().unwrap_or_else(|| key.clone());
                            match client.get_page(&course.canvas_id, &lookup).await {
                                Ok(full) => full.body,
                                Err(e) if e.is_not_found() => None,
                                Err(e) if e.is_auth() => return Err(friendly(&e, "pages")),
                                // Any other failure: keep what we already had.
                                Err(e) => {
                                    body_failed = true;
                                    report.errors.push(format!(
                                        "{}: {}",
                                        course.name,
                                        friendly(&e, "page body")
                                    ));
                                    stored.as_ref().and_then(|s| s.body.clone())
                                }
                            }
                        }
                    };
                    let mut row = map_page(local_id, page, body);
                    // A failed body refresh must be retried next time even when we
                    // retained an older usable body.
                    if body_failed {
                        row.updated_at = stored
                            .as_ref()
                            .map(|s| s.updated_at.clone())
                            .unwrap_or_default();
                    }
                    rows.push(row);
                }
                store
                    .apply_snapshot(st::Snapshot::Pages(local_id, &rows))
                    .map_err(|e| e.to_string())?;
                report.counts.pages += rows.len();
                if report.errors.len() == errors_before {
                    store.record_sync(&key, now).map_err(|e| e.to_string())?;
                    store
                        .record_sync(&format!("{key}|missing"), "")
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(e) if e.is_not_found() => {
                store
                    .record_sync(&format!("{key}|missing"), now)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => return Err(friendly(&e, "pages")),
        }
    }

    Ok(())
}

/// Fetch one course's folders and files *on demand* — the Files tab calls this
/// when it opens.
///
/// Files are the most expensive thing to pull from Canvas (the request is
/// bucketed as costly), and sweeping every course on every sync trips the rate
/// limiter. Canvas itself loads files lazily, and so do we.
pub async fn sync_course_files(
    store: &Store,
    connection: &Connection,
    course_canvas_id: &str,
    course_local_id: st::LocalId,
) -> Result<usize, String> {
    sync_files_with_policy(store, connection, course_canvas_id, course_local_id, true).await
}

pub async fn sync_files_with_policy(
    store: &Store,
    connection: &Connection,
    course_canvas_id: &str,
    course_local_id: st::LocalId,
    force: bool,
) -> Result<usize, String> {
    let client = connection.client().map_err(|e| e.to_string())?;
    let key = format!("{}|course:{course_canvas_id}|files", client.base_url());
    if !force && files_fresh(store, connection, course_canvas_id) {
        return store
            .list_files(course_local_id)
            .map(|rows| rows.len())
            .map_err(|e| e.to_string());
    }
    let now = Utc::now().to_rfc3339();
    let mut count = 0;
    let folder_key = format!("{}|course:{course_canvas_id}|folders", client.base_url());
    if force
        || (!fresh(store, &folder_key, 1800)
            && !fresh(store, &format!("{folder_key}|missing"), 1800))
    {
        match client.list_folders(course_canvas_id).await {
            Ok(folders) => {
                let rows: Vec<_> = folders
                    .iter()
                    .map(|f| map_folder(course_local_id, f, &now))
                    .collect();
                store
                    .apply_snapshot(st::Snapshot::Folders(course_local_id, &rows))
                    .map_err(|e| e.to_string())?;
                store
                    .record_sync(&format!("{folder_key}|missing"), "")
                    .map_err(|e| e.to_string())?;
                store
                    .record_sync(&folder_key, &now)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) if e.is_not_found() => {
                store
                    .record_sync(&format!("{folder_key}|missing"), &now)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => return Err(friendly(&e, "folders")),
        }
    }
    match client.list_files(course_canvas_id).await {
        Ok(files) => {
            let rows: Vec<_> = files
                .iter()
                .map(|f| map_file(course_local_id, f, &now))
                .collect();
            store
                .apply_snapshot(st::Snapshot::Files(course_local_id, &rows))
                .map_err(|e| e.to_string())?;
            store
                .record_sync(&format!("{key}|missing"), "")
                .map_err(|e| e.to_string())?;
            count = rows.len();
        }
        Err(e) if e.is_not_found() => {
            store
                .record_sync(&format!("{key}|missing"), &now)
                .map_err(|e| e.to_string())?;
        }
        Err(e) => return Err(friendly(&e, "files")),
    }
    store.rebuild_search().map_err(|e| e.to_string())?;
    store.record_sync(&key, &now).map_err(|e| e.to_string())?;

    Ok(count)
}

/// Fetch one wiki page by slug/id and store it. Used when a Page is opened but
/// was never synced — notably in courses whose Pages *index* is disabled, where
/// `list_pages` 404s while individual pages still load.
pub async fn fetch_page(
    store: &Store,
    connection: &Connection,
    course_canvas_id: &str,
    key: &str,
) -> Result<(), String> {
    let client = connection.client().map_err(|e| e.to_string())?;
    let local_id = store
        .course_local_id(course_canvas_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "this course is not in the local store".to_string())?;
    let full = client
        .get_page(course_canvas_id, key)
        .await
        .map_err(|e| friendly(&e, "page"))?;
    let body = full.body.clone();
    store
        .upsert_page(&map_page(local_id, &full, body))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn friendly(error: &yunee_canvas::CanvasError, what: &str) -> String {
    if error.is_auth() {
        format!("{what}: Canvas rejected the token — reconnect")
    } else {
        format!("{what}: {error}")
    }
}

// --------------------------------------------------------------------------
// Mapping: Canvas wire types -> local rows
// --------------------------------------------------------------------------

fn course_label(course: &cv::Course) -> String {
    course
        .course_code
        .clone()
        .or_else(|| course.name.clone())
        .unwrap_or_else(|| course.id.clone())
}

fn course_name_for(store: &Store, course_id: st::LocalId) -> String {
    store
        .get_course(course_id)
        .ok()
        .flatten()
        .map(|c| c.name)
        .unwrap_or_else(|| "A course".into())
}

fn map_course(c: &cv::Course, now: &str) -> st::Course {
    let enrollment = c
        .enrollments
        .iter()
        .find(|e| e.enrollment_state.as_deref() == Some("active"))
        .or_else(|| c.enrollments.first());
    st::Course {
        id: 0,
        canvas_id: c.id.clone(),
        name: course_label(c),
        title: c.name.clone().unwrap_or_default(),
        term: c.term.as_ref().and_then(|t| t.name.clone()),
        professor: c.teachers.first().and_then(|t| t.display_name.clone()),
        enrollment_state: enrollment.and_then(|e| e.enrollment_state.clone()),
        current_score: c
            .computed_current_score
            .or_else(|| enrollment.and_then(|e| e.current_score)),
        final_score: c
            .computed_final_score
            .or_else(|| enrollment.and_then(|e| e.final_score)),
        current_grade: c
            .computed_current_grade
            .clone()
            .or_else(|| enrollment.and_then(|e| e.current_grade.clone())),
        final_grade: c
            .computed_final_grade
            .clone()
            .or_else(|| enrollment.and_then(|e| e.final_grade.clone())),
        updated_at: now.to_string(),
    }
}

fn map_assignment(course_id: st::LocalId, a: &cv::Assignment, now: &str) -> st::Assignment {
    let sub = a.submission.as_ref();
    st::Assignment {
        id: 0,
        canvas_id: a.id.clone(),
        course_id,
        name: a
            .name
            .clone()
            .unwrap_or_else(|| "Untitled assignment".into()),
        description: a.description.clone(),
        due_at: a.due_at.clone(),
        unlock_at: a.unlock_at.clone(),
        lock_at: a.lock_at.clone(),
        points_possible: a.points_possible,
        submission_types: serde_json::to_string(&a.submission_types).ok(),
        html_url: a.html_url.clone(),
        quiz_id: a.quiz_id.clone(),
        published: a.published.unwrap_or(true),
        submission_state: sub.and_then(|s| s.workflow_state.clone()),
        submitted_at: sub.and_then(|s| s.submitted_at.clone()),
        graded_at: sub.and_then(|s| s.graded_at.clone()),
        score: sub.and_then(|s| s.score),
        grade: sub.and_then(|s| s.grade.clone()),
        late: sub.and_then(|s| s.late).unwrap_or(false),
        missing: sub.and_then(|s| s.missing).unwrap_or(false),
        excused: sub.and_then(|s| s.excused).unwrap_or(false),
        updated_at: now.to_string(),
    }
}

fn map_announcement(
    course_id: st::LocalId,
    t: &cv::DiscussionTopic,
    now: &str,
) -> st::Announcement {
    let html_body = t.message.clone();
    st::Announcement {
        id: 0,
        canvas_id: t.id.clone(),
        course_id,
        title: t.title.clone().unwrap_or_else(|| "Announcement".into()),
        body: html_body.as_deref().map(html::to_plain).unwrap_or_default(),
        html: html_body,
        posted_at: t.posted_at.clone(),
        author: t.author.as_ref().and_then(|a| a.display_name.clone()),
        read_state: t.read_state.clone(),
        unread_count: t.unread_count.unwrap_or(0),
        local_read_at: None,
        updated_at: now.to_string(),
    }
}

fn map_module(course_id: st::LocalId, m: &cv::Module, now: &str) -> st::Module {
    st::Module {
        id: 0,
        canvas_id: m.id.clone(),
        course_id,
        name: m.name.clone().unwrap_or_else(|| "Module".into()),
        position: m.position,
        unlock_at: m.unlock_at.clone(),
        state: m.state.clone(),
        completed_at: m.completed_at.clone(),
        updated_at: now.to_string(),
    }
}

fn map_module_item(
    course_id: st::LocalId,
    module_id: st::LocalId,
    item: &cv::ModuleItem,
    now: &str,
) -> st::ModuleItem {
    let requirement = item
        .completion_requirement
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok());
    let completed = item
        .completion_requirement
        .as_ref()
        .and_then(|r| r.completed)
        .or(item.completed)
        .unwrap_or(false);
    // A Page item's target is its slug (`page_url`); Canvas leaves `content_id`
    // null for Pages. Keeping the slug in `content_id` makes every item type
    // routable through one column, with no schema change.
    let content_id = item.content_id.clone().or_else(|| item.page_url.clone());
    // An ExternalUrl item's `html_url` is a redirect endpoint, not the link, so
    // prefer the real external URL it carries.
    let html_url = item.external_url.clone().or_else(|| item.html_url.clone());
    st::ModuleItem {
        id: 0,
        canvas_id: item.id.clone(),
        module_id,
        course_id,
        title: item.title.clone().unwrap_or_else(|| "Item".into()),
        item_type: item.item_type.clone(),
        content_id,
        html_url,
        position: item.position,
        completion_requirement: requirement,
        completed,
        updated_at: now.to_string(),
    }
}

fn map_page(course_id: st::LocalId, p: &cv::Page, body: Option<String>) -> st::Page {
    st::Page {
        id: 0,
        // Canvas's page id is globally unique; fall back to the slug.
        canvas_id: p
            .page_id
            .clone()
            .or_else(|| p.url.clone())
            .unwrap_or_else(|| "page".into()),
        course_id,
        page_id: p.page_id.clone(),
        url: p.url.clone(),
        title: p.title.clone().unwrap_or_else(|| "Untitled page".into()),
        body,
        updated_at: p.updated_at.clone().unwrap_or_default(),
    }
}

fn map_folder(course_id: st::LocalId, f: &cv::Folder, now: &str) -> st::Folder {
    st::Folder {
        id: 0,
        canvas_id: f.id.clone(),
        course_id,
        parent_id: f.parent_folder_id.clone(),
        name: f.name.clone().unwrap_or_else(|| "Folder".into()),
        full_name: f.full_name.clone(),
        updated_at: now.to_string(),
    }
}

fn map_file(course_id: st::LocalId, f: &cv::FileEntry, now: &str) -> st::FileEntry {
    st::FileEntry {
        id: 0,
        canvas_id: f.id.clone(),
        course_id,
        folder_id: f.folder_id.clone(),
        display_name: f
            .display_name
            .clone()
            .or_else(|| f.filename.clone())
            .unwrap_or_else(|| "File".into()),
        filename: f.filename.clone(),
        content_type: f.content_type.clone(),
        size: f.size,
        url: f.url.clone(),
        created_at: f.created_at.clone(),
        updated_at: f.updated_at.clone().or_else(|| Some(now.to_string())),
        local_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    // Real HTTP exercises pagination and failed-list handling together with
    // reconciliation, without credentials or a live Canvas account.
    struct CanvasFixture {
        connection: Connection,
        stop: Arc<AtomicBool>,
        hits: Arc<std::sync::Mutex<Vec<String>>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl CanvasFixture {
        fn new(routes: Vec<(&str, u16, serde_json::Value)>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let shutdown = stop.clone();
            let hits = Arc::new(std::sync::Mutex::new(Vec::new()));
            let requests = hits.clone();
            let routes: Vec<_> = routes
                .into_iter()
                .map(|(p, s, b)| (p.to_string(), s, b.to_string()))
                .collect();
            let thread = std::thread::spawn(move || {
                while !shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream
                                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                                .unwrap();
                            let mut request = Vec::new();
                            let mut buffer = [0; 1024];
                            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                                let n = stream.read(&mut buffer).unwrap();
                                if n == 0 {
                                    break;
                                }
                                request.extend_from_slice(&buffer[..n]);
                            }
                            let request = String::from_utf8_lossy(&request);
                            let path = request.split_whitespace().nth(1).unwrap();
                            requests.lock().unwrap().push(path.to_string());
                            let route = routes
                                .iter()
                                .find(|(p, _, _)| path == p || path.split('?').next() == Some(p));
                            let (status, body) = route
                                .map(|(_, s, b)| (*s, b.as_str()))
                                .unwrap_or((404, "{}"));
                            // A special list route makes the first page succeed
                            // and the second fail, proving we never prune on it.
                            let link = if path.starts_with("/api/v1/courses/1/assignments")
                                && body == "[{\"id\":\"paged\"}]"
                            {
                                format!("Link: <http://{address}/failed-page>; rel=\"next\"\r\n")
                            } else {
                                String::new()
                            };
                            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{link}Connection: close\r\n\r\n{body}", body.len()).unwrap();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(2))
                        }
                        Err(e) => panic!("fixture accept failed: {e}"),
                    }
                }
            });
            Self {
                connection: Connection::new(format!("http://{address}"), "fixture-token".into()),
                stop,
                hits,
                thread: Some(thread),
            }
        }
    }

    impl Drop for CanvasFixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    fn seed(store: &Store) -> st::LocalId {
        let course: cv::Course =
            serde_json::from_value(json!({"id": "1", "name": "Course"})).unwrap();
        let cid = store.upsert_course(&map_course(&course, "old")).unwrap();
        let assignment: cv::Assignment =
            serde_json::from_value(json!({"id": "old", "name": "Old assignment"})).unwrap();
        store
            .upsert_assignment(&map_assignment(cid, &assignment, "old"))
            .unwrap();
        store.record_sync("last", "2026-01-01T00:00:00Z").unwrap();
        cid
    }

    #[test]
    fn automatic_sync_skips_fresh_sections_but_explicit_sync_fetches_them() {
        let store = Store::open_in_memory().unwrap();
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 200, json!([])),
            ("/api/v1/courses/1/discussion_topics", 200, json!([])),
            ("/api/v1/courses/1/modules", 200, json!([])),
            ("/api/v1/courses/1/pages", 200, json!([])),
        ]);
        let run = |force| {
            crate::runtime::runtime().block_on(sync_with_policy(&store, &fixture.connection, force))
        };
        assert!(run(false).ok());
        assert_eq!(fixture.hits.lock().unwrap().len(), 5);
        let last_remote = store.last_sync("last").unwrap();
        assert!(run(false).ok());
        assert_eq!(fixture.hits.lock().unwrap().len(), 5);
        assert_eq!(store.last_sync("last").unwrap(), last_remote);
        assert_eq!(
            store.last_sync("last_result").unwrap().as_deref(),
            Some("Cached data is fresh")
        );
        // Only assignments and announcements expire at five minutes.
        for section in ["courses", "course:1|assignments", "course:1|announcements"] {
            store
                .record_sync(
                    &format!(
                        "{}|{section}",
                        fixture.connection.client().unwrap().base_url()
                    ),
                    &(Utc::now() - chrono::Duration::minutes(6)).to_rfc3339(),
                )
                .unwrap();
        }
        assert!(run(false).ok());
        assert_eq!(fixture.hits.lock().unwrap().len(), 8);
        assert!(run(true).ok());
        assert_eq!(fixture.hits.lock().unwrap().len(), 13);
    }

    #[test]
    fn automatic_file_refresh_reuses_cache_and_force_bypasses_it() {
        let store = Store::open_in_memory().unwrap();
        let cid = seed(&store);
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses/1/folders", 200, json!([])),
            (
                "/api/v1/courses/1/files",
                200,
                json!([{"id": "f", "display_name": "Notes"}]),
            ),
        ]);
        let run = |force| {
            crate::runtime::runtime().block_on(sync_files_with_policy(
                &store,
                &fixture.connection,
                "1",
                cid,
                force,
            ))
        };
        assert_eq!(run(false).unwrap(), 1);
        assert_eq!(run(false).unwrap(), 1);
        assert_eq!(fixture.hits.lock().unwrap().len(), 2);
        let key = format!(
            "{}|course:1|files",
            fixture.connection.client().unwrap().base_url()
        );
        store
            .record_sync(
                &key,
                &(Utc::now() - chrono::Duration::minutes(6)).to_rfc3339(),
            )
            .unwrap();
        assert_eq!(run(false).unwrap(), 1);
        // Folder metadata remains fresh for thirty minutes.
        assert_eq!(fixture.hits.lock().unwrap().len(), 3);
        assert_eq!(run(true).unwrap(), 1);
        assert_eq!(fixture.hits.lock().unwrap().len(), 5);
    }

    #[test]
    fn failed_sections_are_not_marked_fresh() {
        let store = Store::open_in_memory().unwrap();
        seed(&store);
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 500, json!({})),
        ]);
        for _ in 0..2 {
            let report = crate::runtime::runtime().block_on(sync_with_policy(
                &store,
                &fixture.connection,
                false,
            ));
            assert!(!report.ok());
        }
        let hits = fixture.hits.lock().unwrap();
        assert_eq!(hits.iter().filter(|p| p.contains("assignments")).count(), 2);
        assert_eq!(
            hits.iter()
                .filter(|p| p.starts_with("/api/v1/courses?"))
                .count(),
            1
        );
    }

    #[test]
    fn failed_later_page_keeps_cached_list_and_success_timestamp() {
        let store = Store::open_in_memory().unwrap();
        let cid = seed(&store);
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            (
                "/api/v1/courses/1/assignments",
                200,
                json!([{"id": "paged"}]),
            ),
            ("/failed-page", 500, json!({})),
        ]);
        let report = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(!report.ok());
        assert_eq!(store.list_assignments(cid).unwrap()[0].canvas_id, "old");
        assert_eq!(
            store.last_sync("last").unwrap().as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert!(store.last_sync("last_attempt").unwrap().is_some());
        assert!(
            store
                .last_sync("last_result")
                .unwrap()
                .unwrap()
                .starts_with("Incomplete:")
        );
    }

    #[test]
    fn successful_empty_lists_prune_and_optional_404_preserves_pages() {
        let store = Store::open_in_memory().unwrap();
        let cid = seed(&store);
        let page: cv::Page =
            serde_json::from_value(json!({"page_id": "7", "url": "syllabus", "body": "Old body"}))
                .unwrap();
        store
            .upsert_page(&map_page(cid, &page, page.body.clone()))
            .unwrap();
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 200, json!([])),
            ("/api/v1/courses/1/discussion_topics", 200, json!([])),
            ("/api/v1/courses/1/modules", 200, json!([])),
        ]);
        let report = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(report.ok(), "{:?}", report.errors);
        assert!(store.list_assignments(cid).unwrap().is_empty());
        assert!(store.get_page(cid, "syllabus").unwrap().is_some());
        assert_ne!(
            store.last_sync("last").unwrap().as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(
            store.last_sync("last_result").unwrap().as_deref(),
            Some("Complete")
        );
    }

    #[test]
    fn omitted_inline_module_items_are_fetched_and_reconciled() {
        let store = Store::open_in_memory().unwrap();
        let cid = seed(&store);
        let module: cv::Module = serde_json::from_value(json!({"id": "m"})).unwrap();
        let mid = store
            .upsert_module(&map_module(cid, &module, "old"))
            .unwrap();
        let old: cv::ModuleItem = serde_json::from_value(json!({"id": "old-item"})).unwrap();
        store
            .upsert_module_item(&map_module_item(cid, mid, &old, "old"))
            .unwrap();
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 200, json!([])),
            ("/api/v1/courses/1/discussion_topics", 200, json!([])),
            (
                "/api/v1/courses/1/modules",
                200,
                json!([{"id": "m", "items_count": 1}]),
            ),
            (
                "/api/v1/courses/1/modules/m/items",
                200,
                json!([{"id": "new-item"}]),
            ),
        ]);
        let report = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(report.ok(), "{:?}", report.errors);
        let items = store.list_module_items(mid).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].canvas_id, "new-item");
    }

    #[test]
    fn repeated_partial_sync_does_not_repeat_announcement_notifications() {
        let store = Store::open_in_memory().unwrap();
        seed(&store);
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 200, json!([])),
            (
                "/api/v1/courses/1/discussion_topics",
                200,
                json!([{"id": "news", "title": "New announcement", "read_state": "unread"}]),
            ),
            ("/api/v1/courses/1/modules", 500, json!({})),
        ]);
        let first = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(!first.ok());
        assert_eq!(first.new_unread.len(), 1);
        let second = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(!second.ok());
        assert!(second.new_unread.is_empty());
        assert_eq!(
            store.last_sync("last").unwrap().as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn failed_page_body_refresh_retains_old_revision_for_retry() {
        let store = Store::open_in_memory().unwrap();
        let cid = seed(&store);
        let page: cv::Page = serde_json::from_value(
            json!({"page_id": "7", "url": "syllabus", "updated_at": "old", "body": "Old body"}),
        )
        .unwrap();
        store
            .upsert_page(&map_page(cid, &page, page.body.clone()))
            .unwrap();
        let fixture = CanvasFixture::new(vec![
            ("/api/v1/courses", 200, json!([{"id": "1"}])),
            ("/api/v1/courses/1/assignments", 200, json!([])),
            ("/api/v1/courses/1/discussion_topics", 200, json!([])),
            ("/api/v1/courses/1/modules", 200, json!([])),
            (
                "/api/v1/courses/1/pages",
                200,
                json!([{"page_id": "7", "url": "syllabus", "updated_at": "new"}]),
            ),
            ("/api/v1/courses/1/pages/syllabus", 500, json!({})),
        ]);
        let report = crate::runtime::runtime().block_on(sync_all(&store, &fixture.connection));
        assert!(!report.ok());
        let page = store.get_page(cid, "syllabus").unwrap().unwrap();
        assert_eq!(page.updated_at, "old");
        assert_eq!(page.body.as_deref(), Some("Old body"));
    }
}
