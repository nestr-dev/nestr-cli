use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::seq::SliceRandom;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::config::CredentialStorage;

/// Placeholder public client id. Replace once the `nestr-cli` OAuth client is
/// registered in Nestr; profiles may override per environment.
pub const DEFAULT_CLIENT_ID: &str = "nestr-cli";

const CLIENT_CONSUMER: &str = "nestr-cli";
const CALLBACK_PORTS: &[u16] = &[21783, 24861, 27654, 31847, 38129];
const SCOPES: &str = "user nest";

/// Raw token response from the token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Persisted token set (file TOML or keyring).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StoredOAuthTokens {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    /// Unix timestamp (seconds) when the access token expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
}

impl StoredOAuthTokens {
    /// Valid if we have >30s of life left. Nestr tokens are opaque (not JWTs),
    /// so we rely solely on the stored expiry.
    pub fn is_valid(&self) -> bool {
        match self.expiry {
            Some(exp) => exp > unix_now_secs() + 30,
            None => false,
        }
    }
}

pub fn tokens_to_stored(t: &TokenResponse) -> StoredOAuthTokens {
    StoredOAuthTokens {
        access_token: t.access_token.clone(),
        refresh_token: t.refresh_token.clone(),
        id_token: t.id_token.clone(),
        expiry: t.expires_in.map(|s| unix_now_secs() + s),
    }
}

pub fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn generate_pkce() -> (String, String) {
    let mut rng = rand::rng();
    let verifier_bytes: [u8; 32] = rng.random();
    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn generate_state() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Percent-encode a string for use as a query parameter value.
/// Uses RFC 3986 unreserved characters; space → %20 (not +).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

fn build_auth_url(
    authorize_url: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    format!(
        "{authorize_url}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        pct_encode(client_id),
        pct_encode(redirect_uri),
        pct_encode(SCOPES),
        pct_encode(code_challenge),
        pct_encode(state),
    )
}

fn bind_callback_listener() -> Result<(TcpListener, u16)> {
    let mut ports = CALLBACK_PORTS.to_vec();
    ports.shuffle(&mut rand::rng());
    for port in ports {
        if let Ok(listener) = TcpListener::bind(format!("127.0.0.1:{port}")) {
            return Ok((listener, port));
        }
    }
    bail!("Could not bind any OAuth callback port (all in use).")
}

fn wait_for_callback_blocking(listener: TcpListener, expected_state: String) -> Result<String> {
    loop {
        let (stream, _) = listener.accept()?;
        if let Some(code) = extract_and_respond(stream, &expected_state)? {
            return Ok(code);
        }
    }
}

fn extract_and_respond(stream: TcpStream, expected_state: &str) -> Result<Option<String>> {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    drop(reader);

    let path = request_line
        .split_whitespace()
        .nth(1)
        .context("malformed HTTP request in OAuth callback")?;
    let Some(query) = path.split('?').nth(1) else {
        send_http_response(&stream, 204, "");
        return Ok(None);
    };
    let params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();

    let (code, returned_state) = match (params.get("code"), params.get("state")) {
        (Some(c), Some(s)) => (c.clone(), s.clone()),
        _ => {
            send_http_response(&stream, 204, "");
            return Ok(None);
        }
    };
    if returned_state != expected_state {
        send_http_response(&stream, 400, "State mismatch.");
        bail!("OAuth state mismatch — possible CSRF, aborting.");
    }
    send_http_response(
        &stream,
        200,
        "<html><body><h2>Authentication successful</h2><p>You may close this tab.</p></body></html>",
    );
    Ok(Some(code))
}

fn send_http_response(mut stream: &TcpStream, status: u16, body: &str) {
    let status_text = match status {
        200 => "200 OK",
        204 => "204 No Content",
        400 => "400 Bad Request",
        _ => "200 OK",
    };
    let response = if body.is_empty() {
        format!("HTTP/1.1 {status_text}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    } else {
        format!(
            "HTTP/1.1 {status_text}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    };
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

async fn post_token_form(token_url: &str, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_url)
        .header("X-Client-Consumer", CLIENT_CONSUMER)
        .form(form)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Token endpoint failed ({status}): {body}");
    }
    resp.json::<TokenResponse>()
        .await
        .context("parsing token response")
}

/// Run the full browser PKCE login. Returns the raw token response.
pub async fn browser_login(
    authorize_url: &str,
    token_url: &str,
    client_id: &str,
) -> Result<TokenResponse> {
    let (verifier, challenge) = generate_pkce();
    let state = generate_state();
    let (listener, port) = bind_callback_listener()?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let auth_url = build_auth_url(authorize_url, client_id, &redirect_uri, &challenge, &state);

    println!("Opening browser for authentication…");
    if open::that(&auth_url).is_err() {
        println!("Could not open a browser. Visit:\n  {auth_url}");
    }

    let code = tokio::time::timeout(
        Duration::from_secs(300),
        tokio::task::spawn_blocking(move || wait_for_callback_blocking(listener, state)),
    )
    .await
    .context("OAuth login timed out after 5 minutes")?
    .context("OAuth callback task failed")??;

    println!("Authorization code received; exchanging for tokens…");
    post_token_form(
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &verifier),
        ],
    )
    .await
}

async fn refresh(token_url: &str, client_id: &str, refresh_token: &str) -> Result<TokenResponse> {
    post_token_form(
        token_url,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
        ],
    )
    .await
}

pub fn store_tokens_keyring(profile: &str, t: &TokenResponse) -> Result<()> {
    let stored = tokens_to_stored(t);
    let json = serde_json::to_string(&stored)?;
    crate::keyring_store::store_secret(profile, "oauth_tokens", &json)
}

pub(crate) fn load_tokens_keyring(profile: &str) -> Result<Option<StoredOAuthTokens>> {
    match crate::keyring_store::get_secret(profile, "oauth_tokens")? {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

/// Return a usable bearer token, refreshing if expired. For file storage,
/// returns `Some(new_tokens)` when refreshed so the caller can persist them.
pub async fn resolve_token(
    profile_name: &str,
    token_url: &str,
    client_id: &str,
    storage: CredentialStorage,
    file_tokens: Option<&StoredOAuthTokens>,
) -> Result<(String, Option<StoredOAuthTokens>)> {
    let current: StoredOAuthTokens = match storage {
        CredentialStorage::OsStore => load_tokens_keyring(profile_name)?.ok_or_else(|| {
            anyhow::anyhow!("No OAuth session for '{profile_name}'. Run `nestr auth login`.")
        })?,
        CredentialStorage::File => file_tokens.cloned().ok_or_else(|| {
            anyhow::anyhow!("No OAuth session for '{profile_name}'. Run `nestr auth login`.")
        })?,
    };

    if current.is_valid() {
        return Ok((current.access_token, None));
    }
    let refresh_token = current.refresh_token.clone().ok_or_else(|| {
        anyhow::anyhow!("OAuth session expired for '{profile_name}'. Run `nestr auth login`.")
    })?;

    let refreshed = refresh(token_url, client_id, &refresh_token).await?;
    match storage {
        CredentialStorage::OsStore => {
            store_tokens_keyring(profile_name, &refreshed)?;
            Ok((refreshed.access_token, None))
        }
        CredentialStorage::File => {
            let stored = tokens_to_stored(&refreshed);
            Ok((stored.access_token.clone(), Some(stored)))
        }
    }
}

/// Read the current refresh token for a profile, from whichever store it uses.
pub fn current_refresh_token(
    profile_name: &str,
    storage: CredentialStorage,
    file_tokens: Option<&StoredOAuthTokens>,
) -> anyhow::Result<Option<String>> {
    let tokens = match storage {
        CredentialStorage::OsStore => load_tokens_keyring(profile_name)?,
        CredentialStorage::File => file_tokens.cloned(),
    };
    Ok(tokens.and_then(|t| t.refresh_token))
}

/// Carries everything the client needs to refresh-and-persist on a 403.
/// Cloneable: the `refresh_token` is shared so a rotation is visible to all clones.
#[derive(Debug, Clone)]
pub struct ReactiveRefresh {
    pub token_url: String,
    pub client_id: String,
    pub profile_name: String,
    pub storage: CredentialStorage,
    pub refresh_token: Arc<RwLock<String>>,
}

impl ReactiveRefresh {
    pub fn new(
        token_url: String,
        client_id: String,
        profile_name: String,
        storage: CredentialStorage,
        refresh_token: String,
    ) -> Self {
        Self {
            token_url,
            client_id,
            profile_name,
            storage,
            refresh_token: Arc::new(RwLock::new(refresh_token)),
        }
    }

    /// Refresh the access token, persist the new set, rotate the refresh token,
    /// and return the fresh access token.
    pub async fn perform(&self) -> anyhow::Result<String> {
        let rt = self.refresh_token.read().await.clone();
        let resp = refresh(&self.token_url, &self.client_id, &rt).await?;
        let stored = tokens_to_stored(&resp);
        match self.storage {
            CredentialStorage::OsStore => store_tokens_keyring(&self.profile_name, &resp)?,
            CredentialStorage::File => {
                crate::config::update_profile_oauth_tokens(&self.profile_name, &stored)?
            }
        }
        if let Some(new_rt) = &resp.refresh_token {
            *self.refresh_token.write().await = new_rt.clone();
        }
        Ok(stored.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};

    #[test]
    fn challenge_is_s256_of_verifier() {
        let (verifier, challenge) = generate_pkce();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
        assert!(!verifier.is_empty());
    }

    #[test]
    fn auth_url_contains_required_params() {
        let url = build_auth_url(
            "https://app.nestr.io/dialog/oauth",
            "nestr-cli",
            "http://127.0.0.1:21783/callback",
            "CHALLENGE",
            "STATE",
        );
        assert!(url.starts_with("https://app.nestr.io/dialog/oauth?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=nestr-cli"));
        assert!(url.contains("code_challenge=CHALLENGE"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=STATE"));
        assert!(url.contains("scope=user%20nest"));
    }

    #[test]
    fn stored_tokens_validity_uses_expiry() {
        let now = unix_now_secs();
        let valid = StoredOAuthTokens {
            access_token: "t".into(),
            refresh_token: None,
            id_token: None,
            expiry: Some(now + 600),
        };
        let expired = StoredOAuthTokens {
            access_token: "t".into(),
            refresh_token: None,
            id_token: None,
            expiry: Some(now.saturating_sub(10)),
        };
        assert!(valid.is_valid());
        assert!(!expired.is_valid());
    }
}
