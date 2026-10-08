//! Canvas pagination.
//!
//! Every Canvas list endpoint paginates and advertises the next page in the
//! `Link` response header (RFC 8288). Foxi ignored it and silently truncated at
//! one page; Yunee follows it to the end.

use reqwest::header::HeaderMap;
use url::Url;

/// Parse the `rel="next"` URL out of a `Link` header, if there is one.
///
/// The header looks like:
/// `<https://school.instructure.com/api/v1/courses?page=2>; rel="next",
///  <...?page=1>; rel="current", <...?page=5>; rel="last"`
pub fn next_link(headers: &HeaderMap) -> Option<Url> {
    let raw = headers.get(reqwest::header::LINK)?.to_str().ok()?;
    for segment in raw.split(',') {
        let mut url: Option<&str> = None;
        let mut rel: Option<&str> = None;
        for piece in segment.split(';') {
            let piece = piece.trim();
            if let Some(rest) = piece.strip_prefix('<') {
                if let Some(end) = rest.find('>') {
                    url = Some(&rest[..end]);
                }
            } else if let Some(rest) = piece.strip_prefix("rel=") {
                rel = Some(rest.trim().trim_matches('"'));
            }
        }
        if rel == Some("next") {
            if let Some(u) = url {
                return Url::parse(u).ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, LINK};

    fn headers(value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(LINK, HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn finds_next() {
        let h = headers(
            r#"<https://s.instructure.com/api/v1/courses?page=2>; rel="next", <https://s.instructure.com/api/v1/courses?page=1>; rel="current", <https://s.instructure.com/api/v1/courses?page=9>; rel="last""#,
        );
        assert_eq!(
            next_link(&h).unwrap().as_str(),
            "https://s.instructure.com/api/v1/courses?page=2"
        );
    }

    #[test]
    fn no_next_when_last_page() {
        let h = headers(
            r#"<https://s.instructure.com/api/v1/courses?page=1>; rel="current", <https://s.instructure.com/api/v1/courses?page=1>; rel="last""#,
        );
        assert!(next_link(&h).is_none());
    }

    #[test]
    fn absent_header_is_none() {
        assert!(next_link(&HeaderMap::new()).is_none());
    }
}
