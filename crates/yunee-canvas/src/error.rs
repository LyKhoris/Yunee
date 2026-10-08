//! Errors from talking to Canvas.

use std::time::Duration;

/// Everything that can go wrong on a Canvas request.
#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
    /// The transport failed (DNS, TLS, timeout, ...).
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid Canvas address: {0}")]
    Url(#[from] url::ParseError),

    /// 401 — the token is dead or wrong for this instance. Tokens are
    /// domain-bound, so this is also what a token-for-the-wrong-school looks
    /// like.
    #[error(
        "Canvas rejected the token (HTTP 401). It may have been revoked, expired, or belong to a different Canvas instance."
    )]
    Unauthorized,

    /// 404 — used for feature detection on older/self-hosted Canvas.
    #[error("Canvas returned 404 for {url} — this endpoint may not exist on this instance")]
    NotFound { url: String },

    /// Any other non-success status.
    #[error("Canvas returned HTTP {status}: {body}")]
    Status { status: u16, body: String },

    /// Rate limiting that did not clear within the retry budget.
    #[error("Canvas is rate limiting this client (gave up after {attempts} attempts)")]
    RateLimited { attempts: u32 },

    #[error("{0}")]
    Message(String),
}

impl CanvasError {
    /// True when the failure is a missing endpoint we can degrade around.
    pub fn is_not_found(&self) -> bool {
        matches!(self, CanvasError::NotFound { .. })
    }

    /// True when the token no longer works, so the UI should ask for a new one.
    pub fn is_auth(&self) -> bool {
        matches!(self, CanvasError::Unauthorized)
    }
}

/// Backoff schedule, honoring an explicit `Retry-After` when present.
pub fn backoff(attempt: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(d) = retry_after {
        return d.min(Duration::from_secs(60));
    }
    // 0.5s, 1s, 2s, 4s, capped.
    let millis = 500u64.saturating_mul(1u64 << attempt.min(5));
    Duration::from_millis(millis.min(8_000))
}
