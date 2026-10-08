//! Finding a school's Canvas host from its name.
//!
//! Canvas exposes an unauthenticated account-domain lookup — the same one its
//! own login page's "find your school" box uses. It lets a user type "csuf"
//! instead of `csufullerton.instructure.com`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::CanvasError;

/// Canvas's central host, which serves the lookup.
const SEARCH_URL: &str = "https://canvas.instructure.com/api/v1/accounts/search";

/// One school the lookup matched.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchoolMatch {
    pub name: String,
    /// The school's Canvas host, e.g. `csufullerton.instructure.com`.
    pub domain: String,
}

/// Search for schools by name (or partial name). Returns up to five matches.
pub async fn search_schools(query: &str) -> Result<Vec<SchoolMatch>, CanvasError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let url = reqwest::Url::parse_with_params(SEARCH_URL, &[("name", query)])?;
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "Yunee/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/LyKhoris/Yunee)"
        ))
        .timeout(Duration::from_secs(15))
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(CanvasError::Status {
            status: response.status().as_u16(),
            body: String::new(),
        });
    }
    Ok(response.json().await?)
}
