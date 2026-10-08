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

use chrono::{Local, Utc};
use yunee_canvas::{CanvasClient, types as cv};
use yunee_store as st;
use yunee_store::{PlannerItem, Store};

use crate::{html, state::Connection};

/// What one sync accomplished.
pub struct SyncReport {
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
    let mut report = SyncReport {
        counts: Default::default(),
        errors: Vec::new(),
        new_unread: Vec::new(),
    };

    let client = match connection.client() {
        Ok(client) => client,
        Err(e) => {
            report
                .errors
                .push(format!("could not build the client: {e}"));
            return report;
        }
    };

    let previous_sync = store.last_sync("last").ok().flatten();
    let now = Utc::now().to_rfc3339();

    let courses = match client.list_courses().await {
        Ok(courses) => courses,
        Err(e) => {
            report.errors.push(friendly(&e, "courses"));
            return report;
        }
    };

    for course in &courses {
        let label = course_label(course);
        match sync_course(store, &client, course, &now, &mut report.counts).await {
            Ok(()) => {}
            Err(e) => report.errors.push(format!("{label}: {e}")),
        }
    }

    sync_planner(store, &client, &now, &mut report.counts, &mut report.errors).await;

    // Finish the durable side.
    let _ = store.rebuild_search();
    let _ = store.record_sync("last", &now);

    // New unread announcements since the last run.
    if let Ok(all) = store.list_all_announcements(50) {
        for announcement in all {
            if !announcement.is_unread() {
                continue;
            }
            let is_new = match &previous_sync {
                Some(prev) => announcement
                    .posted_at
                    .as_deref()
                    .map(|p| p > prev.as_str())
                    .unwrap_or(true),
                None => true,
            };
            if is_new {
                let course = course_name_for(store, announcement.course_id);
                report.new_unread.push((course, announcement.title.clone()));
            }
        }
    }

    report
}

async fn sync_course(
    store: &Store,
    client: &CanvasClient,
    course: &cv::Course,
    now: &str,
    counts: &mut st::SyncCounts,
) -> Result<(), String> {
    let local_id = store
        .upsert_course(&map_course(course, now))
        .map_err(|e| e.to_string())?;
    counts.courses += 1;

    // Assignments (with the caller's submission state).
    match client.list_assignments(&course.id).await {
        Ok(items) => {
            for item in &items {
                let record = map_assignment(local_id, item, now);
                if store.upsert_assignment(&record).is_ok() {
                    counts.assignments += 1;
                }
            }
        }
        Err(e) => return Err(friendly(&e, "assignments")),
    }

    // Announcements.
    match client.list_announcements(&course.id).await {
        Ok(topics) => {
            for topic in &topics {
                let record = map_announcement(local_id, topic, now);
                if store.upsert_announcement(&record).is_ok() {
                    counts.announcements += 1;
                }
            }
        }
        Err(e) => return Err(friendly(&e, "announcements")),
    }

    // Modules + their items.
    match client.list_modules(&course.id).await {
        Ok(modules) => {
            for module in &modules {
                let module_local = match store.upsert_module(&map_module(local_id, module, now)) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                counts.modules += 1;
                for item in &module.items {
                    let _ = store.upsert_module_item(&map_module_item(
                        local_id,
                        module_local,
                        item,
                        now,
                    ));
                }
            }
        }
        Err(e) if e.is_not_found() => {} // older Canvas: skip quietly
        Err(e) => return Err(friendly(&e, "modules")),
    }

    // Folders + files.
    if let Ok(folders) = client.list_folders(&course.id).await {
        for folder in &folders {
            let _ = store.upsert_folder(&map_folder(local_id, folder, now));
        }
    }
    match client.list_files(&course.id).await {
        Ok(files) => {
            for file in &files {
                if store.upsert_file(&map_file(local_id, file, now)).is_ok() {
                    counts.files += 1;
                }
            }
        }
        Err(e) if e.is_not_found() => {}
        Err(e) => return Err(friendly(&e, "files")),
    }

    Ok(())
}

async fn sync_planner(
    store: &Store,
    client: &CanvasClient,
    now: &str,
    counts: &mut st::SyncCounts,
    errors: &mut Vec<String>,
) {
    let today = Local::now().date_naive();
    let start = (today - chrono::Duration::days(14))
        .format("%Y-%m-%d")
        .to_string();
    let end = (today + chrono::Duration::days(45))
        .format("%Y-%m-%d")
        .to_string();
    match client.planner_items(Some(&start), Some(&end)).await {
        Ok(items) => {
            for item in &items {
                if let Some(record) = map_planner(item, now) {
                    if store.upsert_planner_item(&record).is_ok() {
                        counts.planner += 1;
                    }
                }
            }
        }
        Err(e) if e.is_not_found() => {}
        Err(e) => errors.push(friendly(&e, "planner")),
    }
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
        quiz_id: a.quiz_id.map(|q| q.to_string()),
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
    st::ModuleItem {
        id: 0,
        canvas_id: item.id.clone(),
        module_id,
        course_id,
        title: item.title.clone().unwrap_or_else(|| "Item".into()),
        item_type: item.item_type.clone(),
        content_id: item.content_id.clone(),
        html_url: item.html_url.clone(),
        position: item.position,
        completion_requirement: requirement,
        completed,
        updated_at: now.to_string(),
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

fn map_planner(item: &cv::PlannerItem, now: &str) -> Option<PlannerItem> {
    let plannable_type = item
        .plannable_type
        .clone()
        .unwrap_or_else(|| "unknown".into());
    let plannable_id = item.plannable_id.clone();
    let course_id = item.course_id.map(|c| c.to_string());
    let key = format!(
        "{plannable_type}:{}:{}",
        plannable_id.clone().unwrap_or_default(),
        course_id.clone().unwrap_or_else(|| "0".into())
    );

    let plannable = item.plannable.as_ref();
    let title = plannable
        .and_then(|p| p.title.clone().or_else(|| p.name.clone()))
        .unwrap_or_else(|| "Untitled".into());

    let override_ = item.planner_override.as_ref();
    Some(PlannerItem {
        id: 0,
        key,
        plannable_type,
        plannable_id,
        course_id: None, // resolved later by the UI via Canvas course id
        course_name: None,
        title,
        due_at: plannable.and_then(|p| p.due_at.clone()),
        points_possible: plannable.and_then(|p| p.points_possible),
        html_url: item.html_url.clone(),
        completed: override_.and_then(|o| o.marked_complete).unwrap_or(false),
        dismissed: override_.and_then(|o| o.dismissed).unwrap_or(false),
        submission_state: None,
        updated_at: now.to_string(),
    })
}
