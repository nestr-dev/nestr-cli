use std::sync::Arc;

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

    /// Single code path for all verbs. On a 403 with a refresh context, refresh
    /// the token once and retry; a still-403 falls through to the error mapping.
    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<T> {
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
        let text = self.checked_text(resp).await?;
        let json = if text.trim().is_empty() { "{}" } else { &text };
        Ok(serde_json::from_str(json)?)
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
}
