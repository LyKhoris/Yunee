//! Canvas REST wire types — only the fields Yunee actually uses.
//!
//! Deliberately permissive: unknown fields are ignored, optional fields stay
//! optional, and ids go through `ids::*` so number/string differences never
//! break a sync.

use serde::{Deserialize, Serialize};

use crate::ids;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SelfUser {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub login_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Term {
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Teacher {
    pub display_name: Option<String>,
}

/// The student's own enrollment, when `include[]=enrollments` is asked for.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Enrollment {
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub id: Option<String>,
    pub enrollment_state: Option<String>,
    pub current_score: Option<f64>,
    pub final_score: Option<f64>,
    pub current_grade: Option<String>,
    pub final_grade: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Course {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub name: Option<String>,
    pub course_code: Option<String>,
    pub workflow_state: Option<String>,
    pub term: Option<Term>,
    #[serde(default)]
    pub teachers: Vec<Teacher>,
    #[serde(default)]
    pub enrollments: Vec<Enrollment>,
    /// Present when `include[]=total_scores`.
    pub computed_current_score: Option<f64>,
    pub computed_final_score: Option<f64>,
    pub computed_current_grade: Option<String>,
    pub computed_final_grade: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Submission {
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub id: Option<String>,
    pub workflow_state: Option<String>,
    pub submitted_at: Option<String>,
    pub graded_at: Option<String>,
    pub score: Option<f64>,
    pub grade: Option<String>,
    pub attempt: Option<i64>,
    pub late: Option<bool>,
    pub missing: Option<bool>,
    pub excused: Option<bool>,
    pub html_url: Option<String>,
    pub preview_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Assignment {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub name: Option<String>,
    /// HTML.
    pub description: Option<String>,
    pub due_at: Option<String>,
    pub unlock_at: Option<String>,
    pub lock_at: Option<String>,
    pub points_possible: Option<f64>,
    #[serde(default)]
    pub submission_types: Vec<String>,
    pub html_url: Option<String>,
    pub quiz_id: Option<i64>,
    pub published: Option<bool>,
    /// The caller's own submission, when `include[]=submission`.
    pub submission: Option<Submission>,
    pub course_id: Option<i64>,
    pub can_submit: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicAuthor {
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscussionTopic {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub title: Option<String>,
    /// HTML body.
    pub message: Option<String>,
    pub posted_at: Option<String>,
    pub last_reply_at: Option<String>,
    pub author: Option<TopicAuthor>,
    pub read_state: Option<String>,
    pub unread_count: Option<i64>,
    pub discussion_subentry_count: Option<i64>,
    pub html_url: Option<String>,
    pub course_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletionRequirement {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub min_score: Option<f64>,
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModuleItem {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub content_id: Option<String>,
    pub html_url: Option<String>,
    pub position: Option<i64>,
    pub completion_requirement: Option<CompletionRequirement>,
    /// Canvas reports per-item completion inside `completion_requirement`; this
    /// is set by us from the requirement or a module-item lookup.
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Module {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    pub name: Option<String>,
    pub position: Option<i64>,
    pub unlock_at: Option<String>,
    /// `completed` / `started` / `unlocked`.
    pub state: Option<String>,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub items: Vec<ModuleItem>,
    pub course_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub parent_folder_id: Option<String>,
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileEntry {
    #[serde(deserialize_with = "ids::id")]
    pub id: String,
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub folder_id: Option<String>,
    pub display_name: Option<String>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// The response to step 1 of Canvas's 3-step file upload.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileUploadTarget {
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub id: Option<String>,
    pub upload_url: String,
    #[serde(default)]
    pub upload_params: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlannerOverride {
    pub marked_complete: Option<bool>,
    pub dismissed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlannerItem {
    pub plannable_type: Option<String>,
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub plannable_id: Option<String>,
    pub course_id: Option<i64>,
    /// Polymorphic: shape depends on `plannable_type`.
    pub plannable: Option<Plannable>,
    pub html_url: Option<String>,
    pub planner_override: Option<PlannerOverride>,
    pub submissions: Option<serde_json::Value>,
}

/// The common fields across planner plannables (assignment / quiz / calendar).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Plannable {
    #[serde(default, deserialize_with = "ids::opt_id")]
    pub id: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub due_at: Option<String>,
    pub points_possible: Option<f64>,
    pub course_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoItem {
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub assignment: Option<Assignment>,
    pub html_url: Option<String>,
    pub ignore: Option<String>,
    pub course_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivitySummary {
    pub unread_count: Option<i64>,
    #[serde(default, deserialize_with = "ids::opt_id_vec")]
    pub unread_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnreadCount {
    pub unread_count: Option<i64>,
}
