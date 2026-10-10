//! The durable shapes Yunee keeps on disk.
//!
//! These are *local* rows, not Canvas wire types. Every entity carries the
//! Canvas id it came from (`canvas_id`) so a re-sync can upsert rather than
//! duplicate, and a local autoincrement `id` so the app can reference rows
//! without caring where they came from.

use serde::{Deserialize, Serialize};

/// A row's local primary key.
pub type LocalId = i64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub id: LocalId,
    pub canvas_id: String,
    /// Short code, e.g. `ANTH 300`.
    pub name: String,
    /// Full course title.
    pub title: String,
    pub term: Option<String>,
    pub professor: Option<String>,
    pub enrollment_state: Option<String>,
    pub current_score: Option<f64>,
    pub final_score: Option<f64>,
    pub current_grade: Option<String>,
    pub final_grade: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub id: LocalId,
    pub canvas_id: String,
    pub course_id: LocalId,
    pub name: String,
    /// Canvas HTML description.
    pub description: Option<String>,
    pub due_at: Option<String>,
    pub unlock_at: Option<String>,
    pub lock_at: Option<String>,
    pub points_possible: Option<f64>,
    /// JSON array of Canvas submission types (`online_text_entry`, ...).
    pub submission_types: Option<String>,
    pub html_url: Option<String>,
    pub quiz_id: Option<String>,
    pub published: bool,
    // --- the student's submission state, when Canvas included it ---
    pub submission_state: Option<String>,
    pub submitted_at: Option<String>,
    pub graded_at: Option<String>,
    pub score: Option<f64>,
    pub grade: Option<String>,
    pub late: bool,
    pub missing: bool,
    pub excused: bool,
    pub updated_at: String,
}

impl Assignment {
    /// True when the student has turned something in.
    pub fn is_submitted(&self) -> bool {
        matches!(
            self.submission_state.as_deref(),
            Some("submitted") | Some("graded")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announcement {
    pub id: LocalId,
    pub canvas_id: String,
    pub course_id: LocalId,
    pub title: String,
    /// Plain text (HTML stripped) — what the UI shows.
    pub body: String,
    /// Original Canvas HTML.
    pub html: Option<String>,
    pub posted_at: Option<String>,
    pub author: Option<String>,
    /// Canvas's own read state (`read` / `unread`).
    pub read_state: Option<String>,
    pub unread_count: i64,
    /// Yunee's local read marker, set when the user opens it offline.
    pub local_read_at: Option<String>,
    pub updated_at: String,
}

impl Announcement {
    pub fn is_unread(&self) -> bool {
        self.local_read_at.is_none() && self.read_state.as_deref() != Some("read")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub id: LocalId,
    pub canvas_id: String,
    pub course_id: LocalId,
    pub name: String,
    pub position: Option<i64>,
    pub unlock_at: Option<String>,
    pub state: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleItem {
    pub id: LocalId,
    pub canvas_id: String,
    pub module_id: LocalId,
    pub course_id: LocalId,
    pub title: String,
    pub item_type: Option<String>,
    pub content_id: Option<String>,
    pub html_url: Option<String>,
    pub position: Option<i64>,
    /// JSON completion requirement, verbatim.
    pub completion_requirement: Option<String>,
    pub completed: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: LocalId,
    pub canvas_id: String,
    pub course_id: LocalId,
    pub folder_id: Option<String>,
    pub display_name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    /// Canvas download URL — always fetched with the bearer token, never
    /// handed to the browser or trusted as a bare link.
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Set once the file has been downloaded to disk.
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: LocalId,
    pub canvas_id: String,
    pub course_id: LocalId,
    pub parent_id: Option<String>,
    pub name: String,
    pub full_name: Option<String>,
    pub updated_at: String,
}

/// A Canvas wiki page, stored so it can be read offline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: LocalId,
    /// Canvas's page id when it sends one, else the `url` slug.
    pub canvas_id: String,
    pub course_id: LocalId,
    pub page_id: Option<String>,
    /// The stable per-course slug, e.g. `syllabus`.
    pub url: Option<String>,
    pub title: String,
    /// Canvas HTML body.
    pub body: Option<String>,
    pub updated_at: String,
}

impl Page {
    /// True when we hold a body worth rendering.
    pub fn has_body(&self) -> bool {
        self.body
            .as_deref()
            .map(|b| !b.trim().is_empty())
            .unwrap_or(false)
    }
}

/// One hit from the local full-text index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub kind: String,
    pub ref_id: String,
    pub title: String,
    pub snippet: String,
}

/// Counts reported after a sync, for the UI toast.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncCounts {
    pub courses: usize,
    pub assignments: usize,
    pub announcements: usize,
    pub modules: usize,
    pub files: usize,
    pub pages: usize,
}
