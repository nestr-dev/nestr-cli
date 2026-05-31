use reqwest::{header, Client, StatusCode};
use rustls;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{extract_error_detail, NestrError, Result};

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

/// Thin reqwest wrapper pre-configured with Nestr auth + consumer headers.
#[derive(Clone)]
pub struct NestrClient {
    inner: Client,
    api_base: String,
}

impl NestrClient {
    /// `api_base` is e.g. `https://app.nestr.io/api`; `bearer` is the token.
    pub fn new(api_base: impl Into<String>, bearer: &str) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {bearer}"))
                .map_err(|_| NestrError::Auth("invalid token format".into()))?,
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "X-Client-Consumer",
            header::HeaderValue::from_static(CLIENT_CONSUMER),
        );
        // Install the ring crypto provider if no global provider has been set yet.
        // Required because we use `rustls-no-provider` (no provider is bundled by default).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let inner = Client::builder()
            .default_headers(headers)
            .user_agent(concat!("nestr-cli/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            inner,
            api_base: api_base.into(),
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let resp = self
            .inner
            .get(format!("{}{path}", self.api_base))
            .query(params)
            .send()
            .await?;
        let text = self.checked_text(resp).await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let resp = self
            .inner
            .post(format!("{}{path}", self.api_base))
            .json(body)
            .send()
            .await?;
        let text = self.checked_text(resp).await?;
        let json = if text.trim().is_empty() { "{}" } else { &text };
        Ok(serde_json::from_str(json)?)
    }

    pub async fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let resp = self
            .inner
            .patch(format!("{}{path}", self.api_base))
            .json(body)
            .send()
            .await?;
        let text = self.checked_text(resp).await?;
        let json = if text.trim().is_empty() { "{}" } else { &text };
        Ok(serde_json::from_str(json)?)
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .inner
            .delete(format!("{}{path}", self.api_base))
            .send()
            .await?;
        let text = self.checked_text(resp).await?;
        let json = if text.trim().is_empty() { "{}" } else { &text };
        Ok(serde_json::from_str(json)?)
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
