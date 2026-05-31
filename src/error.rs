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
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },
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
    non_empty(&v["message"])
        .or_else(|| non_empty(&v["description"]))
        .or_else(|| non_empty(&v["error"]))
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
