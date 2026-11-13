use serde_json::Value;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NestrError>;

/// Typed errors surfaced by the API client. Commands convert these into
/// `anyhow::Error` via `?`.
#[derive(Debug, Error)]
pub enum NestrError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("plan required: {0}")]
    PlanRequired(String),
    /// `nests activity --since`: the entry to read back to was not found. The CLI
    /// exits with its own code for it (`activity::EXIT_ANCHOR_NOT_FOUND`).
    #[error("{0}")]
    AnchorNotFound(String),
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },
    /// The server has no such route: an older Nestr answers an unknown GET with its
    /// web page and an unknown write with an empty 405. `reply` says which.
    #[error(
        "This Nestr server doesn't support `{method} {path}` yet (it answered with {reply}). \
         This feature needs a newer Nestr server."
    )]
    Unsupported {
        method: String,
        path: String,
        reply: &'static str,
    },
    /// The server answered without honouring an option the user passed, so its
    /// numbers don't match the request and are not shown.
    #[error("{0}")]
    Ignored(String),
    /// A reply that should have been JSON and wasn't. `snippet` is already cleaned
    /// for the terminal and cut short, with its leading ": " (empty when none).
    #[error("unexpected non-JSON reply (HTTP {status}, {content_type}){snippet}")]
    NonJson {
        status: u16,
        content_type: String,
        snippet: String,
    },
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Pull a human-readable detail out of a Nestr error body. Nestr is inconsistent:
/// most 4xx carry `{description}`, insights carry `{status, message}`.
pub fn extract_error_detail(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let non_empty = |val: &Value| val.as_str().filter(|s| !s.is_empty()).map(String::from);
    // Keep a machine code (`"error": "not-member"`) next to its message.
    if let (Some(m), Some(c)) = (non_empty(&v["message"]), non_empty(&v["error"])) {
        if !c.contains(' ') && c != m {
            return Some(format!("{m} [{c}]"));
        }
    }
    non_empty(&v["message"])
        .or_else(|| non_empty(&v["description"]))
        .or_else(|| non_empty(&v["error"]))
}

/// What [`NestrError::Unsupported`] says the server answered with.
pub const REPLY_WEB_PAGE: &str = "its web page";
pub const REPLY_EMPTY_405: &str = "HTTP 405 and an empty body";

/// Whether a reply is a web page: an HTML content type, or a body that opens with a tag.
pub fn is_web_page(content_type: Option<&str>, body: &str) -> bool {
    content_type.is_some_and(|c| c.to_ascii_lowercase().contains("text/html"))
        || opens_with_tag(body)
}

/// Whether a body opens with a tag, as the web app's page (`<!DOCTYPE html>`) does.
/// On a success this alone decides: the label can be wrong, the body can't be JSON.
pub fn opens_with_tag(body: &str) -> bool {
    body.trim_start().starts_with('<')
}

/// A short, terminal-safe, single-line excerpt of a body for an error message.
/// Web pages are never excerpted.
pub fn body_snippet(content_type: Option<&str>, body: &str) -> String {
    if is_web_page(content_type, body) {
        return String::new();
    }
    let clean = body_excerpt(body);
    if clean.is_empty() {
        return clean;
    }
    format!(": {clean}")
}

/// A body cleaned for the terminal, on one line and cut to 120 characters.
pub fn body_excerpt(body: &str) -> String {
    const MAX: usize = 120;
    let clean = crate::render::sanitize(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match clean.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &clean[..cut]),
        None => clean,
    }
}

/// The error for a reply that should have been JSON and wasn't.
pub fn non_json(status: u16, content_type: Option<&str>, body: &str) -> NestrError {
    NestrError::NonJson {
        status,
        content_type: content_type.unwrap_or("no content type").to_string(),
        snippet: body_snippet(content_type, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_message_field() {
        // Nestr insights-style error body
        let body = r#"{"status":"error","message":"Pro plan required"}"#;
        assert_eq!(
            extract_error_detail(body),
            Some("Pro plan required".to_string())
        );
    }

    #[test]
    fn keeps_error_code_beside_message() {
        let body = r#"{"status":"error","error":"not-member","message":"Not a member"}"#;
        assert_eq!(
            extract_error_detail(body),
            Some("Not a member [not-member]".to_string())
        );
    }

    #[test]
    fn extracts_description_field() {
        // Nestr default error body
        let body = r#"{"description":"Nest not found"}"#;
        assert_eq!(
            extract_error_detail(body),
            Some("Nest not found".to_string())
        );
    }

    #[test]
    fn returns_none_for_unstructured_body() {
        assert_eq!(extract_error_detail("plain text error"), None);
        assert_eq!(extract_error_detail(r#"{"other":"x"}"#), None);
    }
}
