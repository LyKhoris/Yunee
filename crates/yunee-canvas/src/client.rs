//! The Canvas REST client.
//!
//! One client per connection. It owns the bearer token, follows `Link`
//! pagination, and backs off when Canvas throttles. It knows nothing about
//! Yunee's storage or UI.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::header::{CONTENT_TYPE, HeaderMap, RETRY_AFTER};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::{CanvasError, backoff};
use crate::pagination::next_link;
use crate::types::*;

/// How many times to retry a throttled request before giving up.
const MAX_RETRIES: u32 = 6;

/// The `Accept` value that asks Canvas to return ids as strings. We still parse
/// tolerantly, but asking reduces surprise.
const STRING_IDS: &str = "application/json+canvas-string-ids";

pub type Result<T> = std::result::Result<T, CanvasError>;

/// A connection to one Canvas instance.
pub struct CanvasClient {
    http: Client,
    base: Url,
    token: String,
    /// Last observed `X-Rate-Limit-Remaining`, in request-cost units.
    remaining: Mutex<Option<f64>>,
    /// Set when Canvas throttles us; every later request waits until it passes,
    /// so one 403/429 slows the whole sync rather than just retrying the one
    /// request that tripped the bucket.
    cooldown: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for CanvasClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never render the token.
        f.debug_struct("CanvasClient")
            .field("base", &self.base.as_str())
            .finish_non_exhaustive()
    }
}

impl CanvasClient {
    /// Build a client. `base_url` may omit the scheme; `https://` is assumed.
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let base = normalize_base(base_url)?;
        let http = Client::builder()
            .user_agent(concat!(
                "Yunee/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/LyKhoris/Yunee)"
            ))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            http,
            base,
            token: token.to_string(),
            remaining: Mutex::new(None),
            cooldown: Mutex::new(None),
        })
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }

    /// Exposed for the connector dialog's "verify" step; callers should not
    /// persist it beyond what the secret store keeps.
    pub fn token(&self) -> &str {
        &self.token
    }

    // ----------------------------------------------------------------------
    // Reads
    // ----------------------------------------------------------------------

    /// Prove the token works and identify whose it is.
    pub async fn verify_token(&self) -> Result<SelfUser> {
        self.get_json(self.endpoint("/users/self")?).await
    }

    pub async fn list_courses(&self) -> Result<Vec<Course>> {
        let mut url = self.endpoint("/courses")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("enrollment_type", "student");
            q.append_pair("enrollment_state", "active");
            q.append_pair("state[]", "available");
            q.append_pair("per_page", "100");
            for include in ["teachers", "term", "enrollments", "total_scores"] {
                q.append_pair("include[]", include);
            }
        }
        self.get_paged(url).await
    }

    pub async fn list_assignments(&self, course_id: &str) -> Result<Vec<Assignment>> {
        let mut url = self.endpoint(&format!("/courses/{course_id}/assignments"))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("per_page", "100");
            q.append_pair("order_by", "due_at");
            q.append_pair("include[]", "submission");
            q.append_pair("include[]", "can_submit");
        }
        self.get_paged(url).await
    }

    pub async fn list_announcements(&self, course_id: &str) -> Result<Vec<DiscussionTopic>> {
        let mut url = self.endpoint(&format!("/courses/{course_id}/discussion_topics"))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("only_announcements", "true");
            q.append_pair("per_page", "100");
        }
        self.get_paged(url).await
    }

    pub async fn list_modules(&self, course_id: &str) -> Result<Vec<Module>> {
        let mut url = self.endpoint(&format!("/courses/{course_id}/modules"))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("per_page", "100");
            q.append_pair("include[]", "items");
        }
        self.get_paged(url).await
    }

    pub async fn list_folders(&self, course_id: &str) -> Result<Vec<Folder>> {
        let mut url = self.endpoint(&format!("/courses/{course_id}/folders"))?;
        url.query_pairs_mut().append_pair("per_page", "100");
        self.get_paged(url).await
    }

    pub async fn list_files(&self, course_id: &str) -> Result<Vec<FileEntry>> {
        let mut url = self.endpoint(&format!("/courses/{course_id}/files"))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("per_page", "100");
            q.append_pair("sort", "updated_at");
            q.append_pair("order", "desc");
        }
        self.get_paged(url).await
    }

    pub async fn planner_items(
        &self,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Vec<PlannerItem>> {
        let mut url = self.endpoint("/planner/items")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("per_page", "100");
            if let Some(s) = start {
                q.append_pair("start_date", s);
            }
            if let Some(e) = end {
                q.append_pair("end_date", e);
            }
        }
        self.get_paged(url).await
    }

    pub async fn todo(&self) -> Result<Vec<TodoItem>> {
        let mut url = self.endpoint("/users/self/todo")?;
        url.query_pairs_mut().append_pair("per_page", "100");
        self.get_paged(url).await
    }

    pub async fn missing_submissions(&self) -> Result<Vec<Assignment>> {
        let mut url = self.endpoint("/users/self/missing_submissions")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("per_page", "100");
            q.append_pair("include[]", "submission");
        }
        self.get_paged(url).await
    }

    pub async fn activity_summary(&self) -> Result<Vec<ActivitySummary>> {
        self.get_json(self.endpoint("/users/self/activity_stream/summary")?)
            .await
    }

    pub async fn unread_conversations(&self) -> Result<UnreadCount> {
        self.get_json(self.endpoint("/conversations/unread_count")?)
            .await
    }

    // ----------------------------------------------------------------------
    // Downloads
    // ----------------------------------------------------------------------

    /// Fetch a Canvas file by the URL in its `FileEntry`. The token is always
    /// attached — Yunee never relies on deprecated verifier links.
    pub async fn download(&self, url: &str) -> Result<Response> {
        let url = Url::parse(url)?;
        let resp = self.http.get(url).bearer_auth(&self.token).send().await?;
        if resp.status().is_success() {
            Ok(resp)
        } else if resp.status() == 401 {
            Err(CanvasError::Unauthorized)
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(CanvasError::Status {
                status,
                body: body.chars().take(300).collect(),
            })
        }
    }

    // ----------------------------------------------------------------------
    // Student writes
    // ----------------------------------------------------------------------

    /// Submit a text-entry assignment.
    pub async fn submit_text(
        &self,
        course_id: &str,
        assignment_id: &str,
        body: &str,
    ) -> Result<Submission> {
        self.submit(
            course_id,
            assignment_id,
            &[
                (
                    "submission[submission_type]".into(),
                    "online_text_entry".into(),
                ),
                ("submission[body]".into(), body.to_string()),
            ],
        )
        .await
    }

    /// Submit a URL assignment.
    pub async fn submit_url(
        &self,
        course_id: &str,
        assignment_id: &str,
        url: &str,
    ) -> Result<Submission> {
        self.submit(
            course_id,
            assignment_id,
            &[
                ("submission[submission_type]".into(), "online_url".into()),
                ("submission[url]".into(), url.to_string()),
            ],
        )
        .await
    }

    /// Submit an uploaded file: the documented 3-step flow.
    pub async fn submit_file(
        &self,
        course_id: &str,
        assignment_id: &str,
        path: &Path,
    ) -> Result<Submission> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| CanvasError::Message(format!("cannot read {}: {e}", path.display())))?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| CanvasError::Message("file has no usable name".into()))?
            .to_string();
        let content_type = guess_content_type(&filename);

        // Step 1 — ask Canvas where to put it.
        let mut url = self.endpoint(&format!(
            "/courses/{course_id}/assignments/{assignment_id}/submissions/self/files"
        ))?;
        let target: FileUploadTarget = self
            .post_form(
                &url,
                &[
                    ("name".into(), filename.clone()),
                    ("size".into(), meta.len().to_string()),
                    ("content_type".into(), content_type.clone()),
                ],
            )
            .await?;
        let _ = &mut url;

        // Step 2 — POST the file to the (usually S3) upload target.
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| CanvasError::Message(format!("cannot open {}: {e}", path.display())))?;
        let part = reqwest::multipart::Part::stream_with_length(file, meta.len())
            .file_name(filename)
            .mime_str(&content_type)
            .map_err(|e| CanvasError::Message(e.to_string()))?;
        let mut form = reqwest::multipart::Form::new();
        for (k, v) in &target.upload_params {
            form = form.text(k.clone(), v.clone());
        }
        form = form.part("file", part);

        let resp = self
            .http
            .post(&target.upload_url)
            .timeout(Duration::from_secs(600))
            .multipart(form)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(CanvasError::Status {
                status,
                body: body.chars().take(300).collect(),
            });
        }
        let uploaded: FileEntry = resp.json().await?;

        // Step 3 — attach the uploaded file id to the submission.
        self.submit(
            course_id,
            assignment_id,
            &[
                ("submission[submission_type]".into(), "online_upload".into()),
                ("submission[file_ids][]".into(), uploaded.id),
            ],
        )
        .await
    }

    async fn submit(
        &self,
        course_id: &str,
        assignment_id: &str,
        form: &[(String, String)],
    ) -> Result<Submission> {
        let url = self.endpoint(&format!(
            "/courses/{course_id}/assignments/{assignment_id}/submissions"
        ))?;
        self.post_form(&url, form).await
    }

    /// Advance a module item that requires `must_mark_done`.
    pub async fn mark_module_item_done(
        &self,
        course_id: &str,
        module_id: &str,
        item_id: &str,
    ) -> Result<()> {
        let url = self.endpoint(&format!(
            "/courses/{course_id}/modules/{module_id}/items/{item_id}/done"
        ))?;
        self.send(|| self.http.put(url.clone())).await?;
        Ok(())
    }

    /// Satisfy a `must_view` requirement for a module item.
    pub async fn mark_module_item_read(
        &self,
        course_id: &str,
        module_id: &str,
        item_id: &str,
    ) -> Result<()> {
        let url = self.endpoint(&format!(
            "/courses/{course_id}/modules/{module_id}/items/{item_id}/mark_read"
        ))?;
        self.send(|| self.http.post(url.clone())).await?;
        Ok(())
    }

    /// Mark an announcement (discussion topic) read on Canvas.
    pub async fn mark_topic_read(&self, course_id: &str, topic_id: &str) -> Result<()> {
        let url = self.endpoint(&format!(
            "/courses/{course_id}/discussion_topics/{topic_id}/read"
        ))?;
        self.send(|| self.http.put(url.clone())).await?;
        Ok(())
    }

    /// Mark or dismiss a planner item without submitting it.
    pub async fn planner_override(
        &self,
        plannable_type: &str,
        plannable_id: &str,
        course_id: &str,
        marked_complete: bool,
    ) -> Result<()> {
        let url = self.endpoint("/planner/overrides")?;
        self.post_form(
            &url,
            &[
                ("plannable_type".into(), plannable_type.to_string()),
                ("plannable_id".into(), plannable_id.to_string()),
                ("course_id".into(), course_id.to_string()),
                ("marked_complete".into(), marked_complete.to_string()),
            ],
        )
        .await
        .map(|_: serde_json::Value| ())
    }

    // ----------------------------------------------------------------------
    // Plumbing
    // ----------------------------------------------------------------------

    /// Absolute URL for an `/api/v1` path on this instance.
    fn endpoint(&self, path: &str) -> Result<Url> {
        let joined = format!(
            "{}/api/v1{}",
            self.base.as_str().trim_end_matches('/'),
            path
        );
        Ok(Url::parse(&joined)?)
    }

    /// GET one page and parse it.
    async fn get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T> {
        let resp = self.send(|| self.http.get(url.clone())).await?;
        Ok(resp.json().await?)
    }

    /// GET every page, following `Link: rel="next"`.
    async fn get_paged<T: DeserializeOwned>(&self, mut url: Url) -> Result<Vec<T>> {
        let mut all = Vec::new();
        loop {
            let resp = self.send(|| self.http.get(url.clone())).await?;
            let next = next_link(resp.headers());
            let mut page: Vec<T> = resp.json().await?;
            all.append(&mut page);
            match next {
                Some(next_url) => url = next_url,
                None => break,
            }
        }
        Ok(all)
    }

    /// POST a urlencoded form and parse the JSON response.
    async fn post_form<T: DeserializeOwned>(
        &self,
        url: &Url,
        form: &[(String, String)],
    ) -> Result<T> {
        let body = encode_form(form);
        let resp = self
            .send(|| {
                self.http
                    .post(url.clone())
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(body.clone())
            })
            .await?;
        Ok(resp.json().await?)
    }

    /// Send a request with auth + throttling + retry, returning a success
    /// response or a classified error.
    async fn send<F>(&self, build: F) -> Result<Response>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut attempt = 0u32;
        loop {
            self.throttle().await;
            let resp = build()
                .bearer_auth(&self.token)
                .header(reqwest::header::ACCEPT, STRING_IDS)
                .send()
                .await?;
            self.note_limits(resp.headers());
            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(CanvasError::Unauthorized);
            }
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(CanvasError::NotFound {
                    url: resp.url().to_string(),
                });
            }
            if status == reqwest::StatusCode::FORBIDDEN
                || status == reqwest::StatusCode::TOO_MANY_REQUESTS
            {
                attempt += 1;
                if attempt > MAX_RETRIES {
                    return Err(CanvasError::RateLimited { attempts: attempt });
                }
                let wait = backoff(attempt, retry_after(resp.headers()));
                *self.cooldown.lock().unwrap() = Some(Instant::now() + wait);
                continue;
            }
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(CanvasError::Status {
                status: code,
                body: body.chars().take(500).collect(),
            });
        }
    }

    /// Remember how much rate-limit budget is left.
    fn note_limits(&self, headers: &HeaderMap) {
        let value = headers
            .get("x-rate-limit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<f64>().ok());
        if let Some(value) = value {
            *self.remaining.lock().unwrap() = Some(value);
        }
    }

    /// Wait if the bucket is drained. A rate-limit response sets a cooldown
    /// that every subsequent request respects, which is what keeps a full
    /// first sync (dozens of requests in a row) from tripping Canvas.
    async fn throttle(&self) {
        let cooldown = {
            let until = *self.cooldown.lock().unwrap();
            until.and_then(|t| t.checked_duration_since(Instant::now()))
        };
        if let Some(wait) = cooldown {
            tokio::time::sleep(wait).await;
            return;
        }
        let remaining = *self.remaining.lock().unwrap();
        if let Some(remaining) = remaining {
            // Slow down as the budget thins out.
            if remaining < 5.0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim().to_string();
    value.parse::<u64>().ok().map(Duration::from_secs)
}

fn encode_form(form: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in form {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

/// Accept `school.instructure.com`, `https://school...`, or with a trailing
/// slash; produce a clean https base.
pub fn normalize_base(input: &str) -> Result<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CanvasError::Message("Canvas address is empty".into()));
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme)?;
    // Drop any path the user pasted (e.g. a `/courses/123` URL) down to origin.
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn guess_content_type(filename: &str) -> String {
    match filename
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("zip") => "application/zip",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bases() {
        assert_eq!(
            normalize_base("school.instructure.com").unwrap().as_str(),
            "https://school.instructure.com/"
        );
        assert_eq!(
            normalize_base("https://school.instructure.com/courses/1")
                .unwrap()
                .as_str(),
            "https://school.instructure.com/"
        );
        assert_eq!(
            normalize_base("  https://x.test/  ").unwrap().as_str(),
            "https://x.test/"
        );
    }

    #[test]
    fn rejects_empty_base() {
        assert!(normalize_base("   ").is_err());
    }

    #[test]
    fn builds_api_endpoints() {
        let client = CanvasClient::new("school.instructure.com", "tok").unwrap();
        assert_eq!(
            client.endpoint("/courses").unwrap().as_str(),
            "https://school.instructure.com/api/v1/courses"
        );
    }

    #[test]
    fn encodes_duplicate_form_keys() {
        let body = encode_form(&[
            ("submission[file_ids][]".into(), "1".into()),
            ("submission[file_ids][]".into(), "2".into()),
        ]);
        assert_eq!(
            body,
            "submission%5Bfile_ids%5D%5B%5D=1&submission%5Bfile_ids%5D%5B%5D=2"
        );
    }
}
