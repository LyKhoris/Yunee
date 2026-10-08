//! `yunee-store` — Yunee's local SQLite memory.
//!
//! Single-user, local, no tenancy: one file holds one student's synced Canvas
//! world. Everything here is headless and unit-tested; the UI and the sync
//! engine are the only callers.
//!
//! Concurrency: a `rusqlite::Connection` is `Send` but not `Sync`, so it lives
//! behind a `Mutex`. The app shares one `Store` (via `Arc`) between the GTK
//! main thread and the background sync thread.

mod models;
mod schema;

pub use models::*;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

/// Anything that can go wrong talking to the local database.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A single-user local database.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (creating if needed) the database at `path` and migrate it.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StoreError::Message(format!("could not create {}: {e}", parent.display()))
            })?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::from_conn(conn)
    }

    /// An ephemeral database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Store {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned lock means another thread panicked mid-write; recovering
        // keeps the app usable rather than cascading the panic everywhere.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Apply the schema. Safe to run on every open.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        for statement in schema::SCHEMA {
            conn.execute_batch(statement)?;
        }
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Courses
    // ----------------------------------------------------------------------

    /// Insert or update a course by its Canvas id. Returns the local id.
    pub fn upsert_course(&self, c: &Course) -> Result<LocalId> {
        let conn = self.conn();
        let id = conn.query_row(
            r#"
            INSERT INTO courses
                (canvas_id, name, title, term, professor, enrollment_state,
                 current_score, final_score, current_grade, final_grade, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(canvas_id) DO UPDATE SET
                name = excluded.name,
                title = excluded.title,
                term = excluded.term,
                professor = excluded.professor,
                enrollment_state = excluded.enrollment_state,
                current_score = excluded.current_score,
                final_score = excluded.final_score,
                current_grade = excluded.current_grade,
                final_grade = excluded.final_grade,
                updated_at = excluded.updated_at
            RETURNING id
            "#,
            params![
                c.canvas_id,
                c.name,
                c.title,
                c.term,
                c.professor,
                c.enrollment_state,
                c.current_score,
                c.final_score,
                c.current_grade,
                c.final_grade,
                c.updated_at,
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn course_local_id(&self, canvas_id: &str) -> Result<Option<LocalId>> {
        let conn = self.conn();
        let id = conn
            .query_row(
                "SELECT id FROM courses WHERE canvas_id = ?1",
                params![canvas_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn list_courses(&self) -> Result<Vec<Course>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT * FROM courses ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], row_to_course)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_course(&self, id: LocalId) -> Result<Option<Course>> {
        let conn = self.conn();
        let course = conn
            .query_row(
                "SELECT * FROM courses WHERE id = ?1",
                params![id],
                row_to_course,
            )
            .optional()?;
        Ok(course)
    }

    // ----------------------------------------------------------------------
    // Assignments
    // ----------------------------------------------------------------------

    pub fn upsert_assignment(&self, a: &Assignment) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO assignments
                (canvas_id, course_id, name, description, due_at, unlock_at, lock_at,
                 points_possible, submission_types, html_url, quiz_id, published,
                 submission_state, submitted_at, graded_at, score, grade,
                 late, missing, excused, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                name = excluded.name,
                description = excluded.description,
                due_at = excluded.due_at,
                unlock_at = excluded.unlock_at,
                lock_at = excluded.lock_at,
                points_possible = excluded.points_possible,
                submission_types = excluded.submission_types,
                html_url = excluded.html_url,
                quiz_id = excluded.quiz_id,
                published = excluded.published,
                submission_state = excluded.submission_state,
                submitted_at = excluded.submitted_at,
                graded_at = excluded.graded_at,
                score = excluded.score,
                grade = excluded.grade,
                late = excluded.late,
                missing = excluded.missing,
                excused = excluded.excused,
                updated_at = excluded.updated_at
            "#,
            params![
                a.canvas_id,
                a.course_id,
                a.name,
                a.description,
                a.due_at,
                a.unlock_at,
                a.lock_at,
                a.points_possible,
                a.submission_types,
                a.html_url,
                a.quiz_id,
                a.published as i64,
                a.submission_state,
                a.submitted_at,
                a.graded_at,
                a.score,
                a.grade,
                a.late as i64,
                a.missing as i64,
                a.excused as i64,
                a.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_assignments(&self, course_id: LocalId) -> Result<Vec<Assignment>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM assignments WHERE course_id = ?1
             ORDER BY (due_at IS NULL), due_at, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![course_id], row_to_assignment)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Every assignment across all courses, soonest due first.
    pub fn list_all_assignments(&self) -> Result<Vec<Assignment>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM assignments ORDER BY (due_at IS NULL), due_at, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], row_to_assignment)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_assignment_by_canvas_id(&self, canvas_id: &str) -> Result<Option<Assignment>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT * FROM assignments WHERE canvas_id = ?1",
                params![canvas_id],
                row_to_assignment,
            )
            .optional()?)
    }

    // ----------------------------------------------------------------------
    // Announcements
    // ----------------------------------------------------------------------

    /// Upsert but preserve a local read marker if the user already read it.
    pub fn upsert_announcement(&self, a: &Announcement) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO announcements
                (canvas_id, course_id, title, body, html, posted_at, author,
                 read_state, unread_count, local_read_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                title = excluded.title,
                body = excluded.body,
                html = excluded.html,
                posted_at = excluded.posted_at,
                author = excluded.author,
                read_state = excluded.read_state,
                unread_count = excluded.unread_count,
                updated_at = excluded.updated_at
            "#,
            params![
                a.canvas_id,
                a.course_id,
                a.title,
                a.body,
                a.html,
                a.posted_at,
                a.author,
                a.read_state,
                a.unread_count,
                a.local_read_at,
                a.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_announcements(&self, course_id: LocalId) -> Result<Vec<Announcement>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM announcements WHERE course_id = ?1
             ORDER BY (posted_at IS NULL), posted_at DESC",
        )?;
        let rows = stmt.query_map(params![course_id], row_to_announcement)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Every announcement across all courses, newest first.
    pub fn list_all_announcements(&self, limit: i64) -> Result<Vec<Announcement>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM announcements ORDER BY (posted_at IS NULL), posted_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_announcement)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn mark_announcement_read(&self, canvas_id: &str, when: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE announcements SET local_read_at = ?2, read_state = 'read' WHERE canvas_id = ?1",
            params![canvas_id, when],
        )?;
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Modules + items
    // ----------------------------------------------------------------------

    pub fn upsert_module(&self, m: &Module) -> Result<LocalId> {
        let conn = self.conn();
        let id = conn.query_row(
            r#"
            INSERT INTO modules
                (canvas_id, course_id, name, position, unlock_at, state, completed_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                name = excluded.name,
                position = excluded.position,
                unlock_at = excluded.unlock_at,
                state = excluded.state,
                completed_at = excluded.completed_at,
                updated_at = excluded.updated_at
            RETURNING id
            "#,
            params![
                m.canvas_id,
                m.course_id,
                m.name,
                m.position,
                m.unlock_at,
                m.state,
                m.completed_at,
                m.updated_at,
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn upsert_module_item(&self, item: &ModuleItem) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO module_items
                (canvas_id, module_id, course_id, title, item_type, content_id,
                 html_url, position, completion_requirement, completed, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(canvas_id) DO UPDATE SET
                module_id = excluded.module_id,
                course_id = excluded.course_id,
                title = excluded.title,
                item_type = excluded.item_type,
                content_id = excluded.content_id,
                html_url = excluded.html_url,
                position = excluded.position,
                completion_requirement = excluded.completion_requirement,
                completed = excluded.completed,
                updated_at = excluded.updated_at
            "#,
            params![
                item.canvas_id,
                item.module_id,
                item.course_id,
                item.title,
                item.item_type,
                item.content_id,
                item.html_url,
                item.position,
                item.completion_requirement,
                item.completed as i64,
                item.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_modules(&self, course_id: LocalId) -> Result<Vec<Module>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM modules WHERE course_id = ?1 ORDER BY (position IS NULL), position, name",
        )?;
        let rows = stmt.query_map(params![course_id], row_to_module)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_module_items(&self, module_id: LocalId) -> Result<Vec<ModuleItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM module_items WHERE module_id = ?1
             ORDER BY (position IS NULL), position, title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![module_id], row_to_module_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_module_item_completed(&self, canvas_id: &str, completed: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE module_items SET completed = ?2 WHERE canvas_id = ?1",
            params![canvas_id, completed as i64],
        )?;
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Folders + files
    // ----------------------------------------------------------------------

    pub fn upsert_folder(&self, f: &Folder) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO folders (canvas_id, course_id, parent_id, name, full_name, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                parent_id = excluded.parent_id,
                name = excluded.name,
                full_name = excluded.full_name,
                updated_at = excluded.updated_at
            "#,
            params![
                f.canvas_id,
                f.course_id,
                f.parent_id,
                f.name,
                f.full_name,
                f.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn upsert_file(&self, f: &FileEntry) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO files
                (canvas_id, course_id, folder_id, display_name, filename, content_type,
                 size, url, created_at, updated_at, local_path)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                folder_id = excluded.folder_id,
                display_name = excluded.display_name,
                filename = excluded.filename,
                content_type = excluded.content_type,
                size = excluded.size,
                url = excluded.url,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                local_path = COALESCE(excluded.local_path, files.local_path)
            "#,
            params![
                f.canvas_id,
                f.course_id,
                f.folder_id,
                f.display_name,
                f.filename,
                f.content_type,
                f.size,
                f.url,
                f.created_at,
                f.updated_at,
                f.local_path,
            ],
        )?;
        Ok(())
    }

    pub fn list_folders(&self, course_id: LocalId) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM folders WHERE course_id = ?1 ORDER BY full_name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![course_id], row_to_folder)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_files(&self, course_id: LocalId) -> Result<Vec<FileEntry>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM files WHERE course_id = ?1 ORDER BY display_name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![course_id], row_to_file)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_file(&self, canvas_id: &str) -> Result<Option<FileEntry>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT * FROM files WHERE canvas_id = ?1",
                params![canvas_id],
                row_to_file,
            )
            .optional()?)
    }

    pub fn set_file_local_path(&self, canvas_id: &str, path: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE files SET local_path = ?2 WHERE canvas_id = ?1",
            params![canvas_id, path],
        )?;
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Pages
    // ----------------------------------------------------------------------

    pub fn upsert_page(&self, p: &Page) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO pages
                (canvas_id, course_id, page_id, url, title, body, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(canvas_id) DO UPDATE SET
                course_id = excluded.course_id,
                page_id = excluded.page_id,
                url = excluded.url,
                title = excluded.title,
                body = excluded.body,
                updated_at = excluded.updated_at
            "#,
            params![
                p.canvas_id,
                p.course_id,
                p.page_id,
                p.url,
                p.title,
                p.body,
                p.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_pages(&self, course_id: LocalId) -> Result<Vec<Page>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM pages WHERE course_id = ?1 ORDER BY title COLLATE NOCASE")?;
        let rows = stmt.query_map(params![course_id], row_to_page)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Find a page by any of its identifiers — Canvas page id, slug, or the
    /// stored key. Module items name pages by id while links name them by slug,
    /// so both have to work.
    pub fn get_page(&self, course_id: LocalId, key: &str) -> Result<Option<Page>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT * FROM pages
                 WHERE course_id = ?1
                   AND (canvas_id = ?2 OR page_id = ?2 OR url = ?2)
                 LIMIT 1",
                params![course_id, key],
                row_to_page,
            )
            .optional()?)
    }

    // ----------------------------------------------------------------------
    // Planner
    // ----------------------------------------------------------------------

    pub fn upsert_planner_item(&self, p: &PlannerItem) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO planner_items
                (key, plannable_type, plannable_id, course_id, course_name, title,
                 due_at, points_possible, html_url, completed, dismissed,
                 submission_state, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(key) DO UPDATE SET
                plannable_type = excluded.plannable_type,
                plannable_id = excluded.plannable_id,
                course_id = excluded.course_id,
                course_name = excluded.course_name,
                title = excluded.title,
                due_at = excluded.due_at,
                points_possible = excluded.points_possible,
                html_url = excluded.html_url,
                completed = excluded.completed,
                dismissed = excluded.dismissed,
                submission_state = excluded.submission_state,
                updated_at = excluded.updated_at
            "#,
            params![
                p.key,
                p.plannable_type,
                p.plannable_id,
                p.course_id,
                p.course_name,
                p.title,
                p.due_at,
                p.points_possible,
                p.html_url,
                p.completed as i64,
                p.dismissed as i64,
                p.submission_state,
                p.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_planner_items(&self) -> Result<Vec<PlannerItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM planner_items WHERE dismissed = 0
             ORDER BY (due_at IS NULL), due_at, title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], row_to_planner)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_planner_completed(&self, key: &str, completed: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE planner_items SET completed = ?2 WHERE key = ?1",
            params![key, completed as i64],
        )?;
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Sync bookkeeping + settings
    // ----------------------------------------------------------------------

    pub fn record_sync(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO sync_state (key, value, updated_at) VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
            params![key, value, now_iso()],
        )?;
        Ok(())
    }

    pub fn last_sync(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT value FROM sync_state WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    // ----------------------------------------------------------------------
    // Search
    // ----------------------------------------------------------------------

    /// Rebuild the full-text index from the current rows. Cheap at this scale
    /// (one student), so it just runs after each sync instead of tracking
    /// per-row triggers.
    pub fn rebuild_search(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            r#"
            DELETE FROM search;
            INSERT INTO search (kind, ref_id, title, body)
                SELECT 'course', canvas_id, name, COALESCE(title, '') FROM courses;
            INSERT INTO search (kind, ref_id, title, body)
                SELECT 'assignment', canvas_id, name, COALESCE(description, '') FROM assignments;
            INSERT INTO search (kind, ref_id, title, body)
                SELECT 'announcement', canvas_id, title, body FROM announcements;
            INSERT INTO search (kind, ref_id, title, body)
                SELECT 'file', canvas_id, display_name, COALESCE(filename, '') FROM files;
            INSERT INTO search (kind, ref_id, title, body)
                SELECT 'page', canvas_id, title, COALESCE(body, '') FROM pages;
            "#,
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let match_query = fts_query(query);
        if match_query.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT kind, ref_id, title, snippet(search, 3, '‹', '›', '…', 12)
             FROM search WHERE search MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_query, limit], |row| {
            Ok(SearchHit {
                kind: row.get(0)?,
                ref_id: row.get(1)?,
                title: row.get(2)?,
                snippet: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

/// Turn a user's free text into a safe FTS5 prefix query. Each word becomes a
/// quoted prefix term (`"anth"*`), which side-steps FTS operators a user might
/// type by accident.
fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|t| t.chars().any(|c| c.is_alphanumeric()))
        .map(|t| {
            let escaped = t.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// Explicit row mappers. Column names (not positions) keep these readable and
// robust against column reordering.

fn row_to_course(row: &rusqlite::Row<'_>) -> rusqlite::Result<Course> {
    Ok(Course {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        name: row.get("name")?,
        title: row.get("title")?,
        term: row.get("term")?,
        professor: row.get("professor")?,
        enrollment_state: row.get("enrollment_state")?,
        current_score: row.get("current_score")?,
        final_score: row.get("final_score")?,
        current_grade: row.get("current_grade")?,
        final_grade: row.get("final_grade")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_assignment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Assignment> {
    Ok(Assignment {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        due_at: row.get("due_at")?,
        unlock_at: row.get("unlock_at")?,
        lock_at: row.get("lock_at")?,
        points_possible: row.get("points_possible")?,
        submission_types: row.get("submission_types")?,
        html_url: row.get("html_url")?,
        quiz_id: row.get("quiz_id")?,
        published: row.get::<_, i64>("published")? != 0,
        submission_state: row.get("submission_state")?,
        submitted_at: row.get("submitted_at")?,
        graded_at: row.get("graded_at")?,
        score: row.get("score")?,
        grade: row.get("grade")?,
        late: row.get::<_, i64>("late")? != 0,
        missing: row.get::<_, i64>("missing")? != 0,
        excused: row.get::<_, i64>("excused")? != 0,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_announcement(row: &rusqlite::Row<'_>) -> rusqlite::Result<Announcement> {
    Ok(Announcement {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        title: row.get("title")?,
        body: row.get("body")?,
        html: row.get("html")?,
        posted_at: row.get("posted_at")?,
        author: row.get("author")?,
        read_state: row.get("read_state")?,
        unread_count: row.get("unread_count")?,
        local_read_at: row.get("local_read_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_module(row: &rusqlite::Row<'_>) -> rusqlite::Result<Module> {
    Ok(Module {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        name: row.get("name")?,
        position: row.get("position")?,
        unlock_at: row.get("unlock_at")?,
        state: row.get("state")?,
        completed_at: row.get("completed_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_module_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModuleItem> {
    Ok(ModuleItem {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        module_id: row.get("module_id")?,
        course_id: row.get("course_id")?,
        title: row.get("title")?,
        item_type: row.get("item_type")?,
        content_id: row.get("content_id")?,
        html_url: row.get("html_url")?,
        position: row.get("position")?,
        completion_requirement: row.get("completion_requirement")?,
        completed: row.get::<_, i64>("completed")? != 0,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        parent_id: row.get("parent_id")?,
        name: row.get("name")?,
        full_name: row.get("full_name")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_page(row: &rusqlite::Row<'_>) -> rusqlite::Result<Page> {
    Ok(Page {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        page_id: row.get("page_id")?,
        url: row.get("url")?,
        title: row.get("title")?,
        body: row.get("body")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileEntry> {
    Ok(FileEntry {
        id: row.get("id")?,
        canvas_id: row.get("canvas_id")?,
        course_id: row.get("course_id")?,
        folder_id: row.get("folder_id")?,
        display_name: row.get("display_name")?,
        filename: row.get("filename")?,
        content_type: row.get("content_type")?,
        size: row.get("size")?,
        url: row.get("url")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        local_path: row.get("local_path")?,
    })
}

fn row_to_planner(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlannerItem> {
    Ok(PlannerItem {
        id: row.get("id")?,
        key: row.get("key")?,
        plannable_type: row.get("plannable_type")?,
        plannable_id: row.get("plannable_id")?,
        course_id: row.get("course_id")?,
        course_name: row.get("course_name")?,
        title: row.get("title")?,
        due_at: row.get("due_at")?,
        points_possible: row.get("points_possible")?,
        html_url: row.get("html_url")?,
        completed: row.get::<_, i64>("completed")? != 0,
        dismissed: row.get::<_, i64>("dismissed")? != 0,
        submission_state: row.get("submission_state")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course(canvas_id: &str, name: &str) -> Course {
        Course {
            id: 0,
            canvas_id: canvas_id.into(),
            name: name.into(),
            title: format!("{name} — full title"),
            term: Some("Fall 2026".into()),
            professor: Some("Dr. Ada".into()),
            enrollment_state: Some("active".into()),
            current_score: Some(88.0),
            final_score: None,
            current_grade: Some("B+".into()),
            final_grade: None,
            updated_at: now_iso(),
        }
    }

    fn assignment(course_id: LocalId, canvas_id: &str, name: &str) -> Assignment {
        Assignment {
            id: 0,
            canvas_id: canvas_id.into(),
            course_id,
            name: name.into(),
            description: Some("<p>Read chapter 3</p>".into()),
            due_at: Some("2026-10-20T07:00:00Z".into()),
            unlock_at: None,
            lock_at: None,
            points_possible: Some(10.0),
            submission_types: Some("[\"online_text_entry\"]".into()),
            html_url: Some("https://canvas.example/assignments/1".into()),
            quiz_id: None,
            published: true,
            submission_state: Some("unsubmitted".into()),
            submitted_at: None,
            graded_at: None,
            score: None,
            grade: None,
            late: false,
            missing: true,
            excused: false,
            updated_at: now_iso(),
        }
    }

    #[test]
    fn migrate_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        store.migrate().unwrap();
        store.migrate().unwrap();
    }

    #[test]
    fn course_upsert_is_by_canvas_id() {
        let store = Store::open_in_memory().unwrap();
        let id1 = store.upsert_course(&course("101", "ANTH 300")).unwrap();
        let mut again = course("101", "ANTH 300");
        again.professor = Some("Dr. Grace".into());
        let id2 = store.upsert_course(&again).unwrap();
        assert_eq!(id1, id2, "same canvas id must reuse the local row");
        let all = store.list_courses().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].professor.as_deref(), Some("Dr. Grace"));
    }

    #[test]
    fn assignment_round_trips_and_searches() {
        let store = Store::open_in_memory().unwrap();
        let cid = store.upsert_course(&course("101", "ANTH 300")).unwrap();
        store
            .upsert_assignment(&assignment(cid, "500", "Reading response 3"))
            .unwrap();
        let list = store.list_assignments(cid).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].missing);
        assert!(!list[0].is_submitted());

        store.rebuild_search().unwrap();
        let hits = store.search("reading", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "assignment");
    }

    #[test]
    fn announcement_read_marker_survives_resync() {
        let store = Store::open_in_memory().unwrap();
        let cid = store.upsert_course(&course("101", "ANTH 300")).unwrap();
        let mut a = Announcement {
            id: 0,
            canvas_id: "900".into(),
            course_id: cid,
            title: "Exam moved".into(),
            body: "Moved to Friday".into(),
            html: Some("<p>Moved to Friday</p>".into()),
            posted_at: Some("2026-10-05T12:00:00Z".into()),
            author: Some("Dr. Ada".into()),
            read_state: Some("unread".into()),
            unread_count: 1,
            local_read_at: None,
            updated_at: now_iso(),
        };
        store.upsert_announcement(&a).unwrap();
        store
            .mark_announcement_read("900", "2026-10-06T00:00:00Z")
            .unwrap();
        // A later sync pushes the same announcement again.
        a.body = "Moved to Friday (updated)".into();
        store.upsert_announcement(&a).unwrap();
        let list = store.list_announcements(cid).unwrap();
        assert_eq!(list[0].body, "Moved to Friday (updated)");
        assert_eq!(
            list[0].local_read_at.as_deref(),
            Some("2026-10-06T00:00:00Z")
        );
        assert!(!list[0].is_unread());
    }

    #[test]
    fn planner_upsert_is_by_key() {
        let store = Store::open_in_memory().unwrap();
        let item = PlannerItem {
            id: 0,
            key: "assignment:500:101".into(),
            plannable_type: "assignment".into(),
            plannable_id: Some("500".into()),
            course_id: None,
            course_name: Some("ANTH 300".into()),
            title: "Reading response 3".into(),
            due_at: Some("2026-10-20T07:00:00Z".into()),
            points_possible: Some(10.0),
            html_url: None,
            completed: false,
            dismissed: false,
            submission_state: Some("unsubmitted".into()),
            updated_at: now_iso(),
        };
        store.upsert_planner_item(&item).unwrap();
        store
            .set_planner_completed("assignment:500:101", true)
            .unwrap();
        let all = store.list_planner_items().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].completed);
    }

    #[test]
    fn page_upsert_and_lookup_by_id_or_slug() {
        let store = Store::open_in_memory().unwrap();
        let cid = store.upsert_course(&course("101", "ANTH 300")).unwrap();
        let page = |canvas_id: &str, page_id: Option<&str>, url: &str| Page {
            id: 0,
            canvas_id: canvas_id.into(),
            course_id: cid,
            page_id: page_id.map(str::to_string),
            url: Some(url.into()),
            title: "Syllabus".into(),
            body: Some("<p>Read chapter 3</p>".into()),
            updated_at: now_iso(),
        };
        store
            .upsert_page(&page("777", Some("777"), "syllabus"))
            .unwrap();
        // Found by page id (how a module item names it)…
        assert!(store.get_page(cid, "777").unwrap().is_some());
        // …and by slug (how a link names it).
        assert!(store.get_page(cid, "syllabus").unwrap().is_some());
        assert!(store.get_page(cid, "missing").unwrap().is_none());

        store.rebuild_search().unwrap();
        let hits = store.search("chapter", 10).unwrap();
        assert!(hits.iter().any(|h| h.kind == "page"));
    }

    #[test]
    fn fts_query_is_operator_safe() {
        assert_eq!(fts_query("hello world"), "\"hello\"* \"world\"*");
        // A bare FTS operator is treated as a literal word, not syntax.
        assert_eq!(fts_query("OR"), "\"OR\"*");
        assert_eq!(fts_query("a\"b"), "\"a\"\"b\"*");
        // Punctuation-only input produces no terms, so no query crashes.
        assert_eq!(fts_query("&& !!"), "");
    }
}
