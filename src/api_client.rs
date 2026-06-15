use std::sync::Arc;
use std::time::Duration;

use reqwest::{header, Client, Method, StatusCode};
use rustls;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::error::{extract_error_detail, NestrError, Result};
use crate::oauth::ReactiveRefresh;

const CLIENT_CONSUMER: &str = "nestr-cli";

/// Defensively unwrap Nestr's `{status, data, meta?, links?}` envelope.
/// Only unwraps when `data` sits alongside an envelope sibling (`status`/`meta`/`links`);
/// otherwise returns the value untouched. Call per-endpoint — never blindly on `/users/me`.
pub fn unwrap_data(v: Value) -> (Value, Option<Value>, Option<Value>) {
    if let Value::Object(mut map) = v {
        let wrapped = map.contains_key("data")
            && (map.contains_key("status")
                || map.contains_key("meta")
                || map.contains_key("links"));
        if wrapped {
            let data = map.remove("data").unwrap_or(Value::Null);
            let meta = map.remove("meta");
            let links = map.remove("links");
            return (data, meta, links);
        }
        return (Value::Object(map), None, None);
    }
    (v, None, None)
}

/// Reject path traversal, query/fragment injection, and out-of-alphabet segments
/// in an API request path. Every request flows through `request_text`, and IDs are
/// interpolated into `path` by callers, so this single chokepoint stops a crafted
/// ID (e.g. one copied by an agent from attacker-authored API content) from
/// retargeting the request to a different route, injecting a query, or escaping
/// via `..`. Allowed per segment: ASCII alphanumerics plus `_`, `-`, and `,`
/// (comma for the multi-id `/nests/{ids}` route). Query params are added separately
/// via reqwest `.query()`, never here. (SEC-2, SEC-16)
fn validate_path(path: &str) -> Result<()> {
    for seg in path.split('/') {
        if seg.is_empty() {
            continue;
        }
        if seg == "." || seg == ".." {
            return Err(NestrError::Validation(format!(
                "illegal path segment '{seg}' in request path '{path}'"
            )));
        }
        if !seg
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b','))
        {
            return Err(NestrError::Validation(format!(
                "illegal characters in id/path segment '{seg}' (request path '{path}')"
            )));
        }
    }
    Ok(())
}

/// Thin reqwest wrapper. The bearer lives in a shared cell so a reactive refresh
/// can swap it mid-flight; consumer + content-type headers are baked in.
#[derive(Clone)]
pub struct NestrClient {
    inner: Client,
    api_base: String,
    auth: Arc<RwLock<String>>,
    refresh: Option<ReactiveRefresh>,
}

impl NestrClient {
    /// No reactive refresh (API-key profiles, tests).
    pub fn new(api_base: impl Into<String>, bearer: &str) -> Result<Self> {
        Self::with_refresh(api_base, bearer, None)
    }

    /// With an optional reactive-refresh context (OAuth profiles).
    pub fn with_refresh(
        api_base: impl Into<String>,
        bearer: &str,
        refresh: Option<ReactiveRefresh>,
    ) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "X-Client-Consumer",
            header::HeaderValue::from_static(CLIENT_CONSUMER),
        );
        let _ = rustls::crypto::ring::default_provider().install_default();
        let inner = Client::builder()
            .default_headers(headers)
            .user_agent(concat!("nestr-cli/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            inner,
            api_base: api_base.into(),
            auth: Arc::new(RwLock::new(bearer.to_string())),
            refresh,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        self.request(Method::GET, path, params, None).await
    }

    pub async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.request(Method::POST, path, &[], Some(body)).await
    }

    pub async fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.request(Method::PATCH, path, &[], Some(body)).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request(Method::DELETE, path, &[], None).await
    }

    /// DELETE with a JSON body (e.g. propose-delete sends `{_id}`).
    pub async fn delete_body<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.request(Method::DELETE, path, &[], Some(body)).await
    }

    /// Send a request (with one 403-refresh-retry) and return the raw checked body.
    async fn request_text(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<String> {
        validate_path(path)?;
        let url = format!("{}{path}", self.api_base);
        let resp = self.send(&method, &url, query, body).await?;
        // On a 403 with a refresh context, refresh the token once and retry.
        let resp = match (resp.status(), &self.refresh) {
            (StatusCode::FORBIDDEN, Some(r)) => {
                let new_token = r
                    .perform()
                    .await
                    .map_err(|e| NestrError::Auth(format!("token refresh failed: {e}")))?;
                *self.auth.write().await = new_token;
                self.send(&method, &url, query, body).await?
            }
            _ => resp,
        };
        self.checked_text(resp).await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<T> {
        let text = self.request_text(method, path, query, body).await?;
        let json = if text.trim().is_empty() { "{}" } else { &text };
        Ok(serde_json::from_str(json)?)
    }

    /// DELETE returning the raw checked body (no JSON parse) — for routes that
    /// respond with a bare string like `"success"`.
    pub async fn delete_text(&self, path: &str) -> Result<String> {
        self.request_text(Method::DELETE, path, &[], None).await
    }

    async fn send(
        &self,
        method: &Method,
        url: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<reqwest::Response> {
        let token = self.auth.read().await.clone();
        let mut rb = self
            .inner
            .request(method.clone(), url)
            .query(query)
            .bearer_auth(token);
        if let Some(b) = body {
            rb = rb.json(b);
        }
        Ok(rb.send().await?)
    }

    /// Validate status and return the body text, mapping Nestr's status codes.
    async fn checked_text(&self, resp: reqwest::Response) -> Result<String> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.text().await?);
        }
        let code = status.as_u16();
        let body = resp.text().await.unwrap_or_default();
        let detail = extract_error_detail(&body);
        let msg = detail.unwrap_or_else(|| body.clone());
        match status {
            // Nestr uses 403 for auth failures too (401 is not used).
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(NestrError::Permission(
                format!("{msg}. Check your token/scopes (run `nestr auth status`)."),
            )),
            StatusCode::NOT_FOUND => Err(NestrError::NotFound(msg)),
            StatusCode::PAYMENT_REQUIRED => Err(NestrError::PlanRequired(msg)),
            StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST => {
                Err(NestrError::Validation(msg))
            }
            _ => Err(NestrError::Api {
                status: code,
                message: msg,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unwrap_data_pulls_data_when_wrapped() {
        let v = json!({"status":"ok","data":[{"_id":"a"}],"meta":{"total":1},"links":{"next":"x"}});
        let (data, meta, links) = unwrap_data(v);
        assert_eq!(data, json!([{"_id":"a"}]));
        assert_eq!(meta, Some(json!({"total":1})));
        assert_eq!(links, Some(json!({"next":"x"})));
    }

    #[test]
    fn unwrap_data_passes_through_bare_object() {
        let v = json!({"_id":"u1","profile":{"fullName":"A"}});
        let (data, meta, links) = unwrap_data(v.clone());
        assert_eq!(data, v);
        assert!(meta.is_none() && links.is_none());
    }

    #[test]
    fn unwrap_data_passes_through_bare_array() {
        let v = json!([{"_id":"a"},{"_id":"b"}]);
        let (data, _, _) = unwrap_data(v.clone());
        assert_eq!(data, v);
    }

    #[test]
    fn unwrap_data_keeps_object_that_only_has_data_key() {
        // A real nest could legitimately have a `data` field with no envelope siblings.
        let v = json!({"data":{"k":1}});
        let (data, _, _) = unwrap_data(v.clone());
        assert_eq!(data, v);
    }

    #[test]
    fn validate_path_accepts_normal_routes() {
        assert!(validate_path("/nests/abc/search").is_ok());
        assert!(validate_path("/workspaces/ws1/users/u-1/groups").is_ok());
        assert!(validate_path("/nests/a,b,c").is_ok()); // multi-id route
        assert!(validate_path("/users/me/notifications/mark-all-read").is_ok());
    }

    #[test]
    fn validate_path_rejects_traversal_and_injection() {
        assert!(validate_path("/nests/../workspaces/W/webhooks/X").is_err()); // SEC-2
        assert!(validate_path("/nests/abc?x=1").is_err()); // query injection
        assert!(validate_path("/nests/abc#frag").is_err());
        assert!(validate_path("/nests/a b").is_err()); // space
        assert!(validate_path("/nests/.").is_err());
    }
}
