use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::oauth::StoredOAuthTokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    #[default]
    OAuth,
    ApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStorage {
    #[default]
    File,
    OsStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
        }
    }
}

/// A named profile. `workspace = profile`: each profile pins one identity and
/// one workspace. `host` is the single source of truth for all URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub auth: AuthKind,
    #[serde(default)]
    pub credential_storage: CredentialStorage,
    /// e.g. "https://app.nestr.io" (prod), "http://localhost:4001" (local dev).
    pub host: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_token_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_authorize_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_tokens: Option<StoredOAuthTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_output_format: Option<OutputFormat>,
}

impl Profile {
    fn host_trimmed(&self) -> &str {
        self.host.trim_end_matches('/')
    }
    /// `{host}/api`
    pub fn api_base(&self) -> String {
        format!("{}/api", self.host_trimmed())
    }
    /// `{host}/oauth/token` unless overridden.
    pub fn token_url(&self) -> String {
        self.oauth_token_url
            .clone()
            .unwrap_or_else(|| format!("{}/oauth/token", self.host_trimmed()))
    }
    /// `{host}/dialog/oauth` unless overridden.
    pub fn authorize_url(&self) -> String {
        self.oauth_authorize_url
            .clone()
            .unwrap_or_else(|| format!("{}/dialog/oauth", self.host_trimmed()))
    }
    /// Resolved OAuth client id: profile override, else the built-in default.
    pub fn client_id(&self) -> String {
        self.oauth_client_id
            .clone()
            .unwrap_or_else(|| crate::oauth::DEFAULT_CLIENT_ID.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_profile")]
    pub default_profile: String,
    #[serde(default)]
    pub default_output_format: OutputFormat,
}

fn default_profile() -> String {
    "default".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_profile: default_profile(),
            default_output_format: OutputFormat::default(),
        }
    }
}

/// Pure helper so the dir logic is testable without touching the real HOME.
pub fn config_dir_in(nestr_home: Option<PathBuf>) -> PathBuf {
    let base = nestr_home
        .or_else(|| std::env::var_os("NESTR_HOME").map(PathBuf::from))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".nestr")
}

pub fn config_dir() -> PathBuf {
    config_dir_in(None)
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

pub fn profile_file(name: &str) -> PathBuf {
    profiles_dir().join(format!("{name}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_uses_nestr_home_override() {
        let dir = config_dir_in(Some("/tmp/fake".into()));
        assert_eq!(dir, std::path::PathBuf::from("/tmp/fake/.nestr"));
    }

    #[test]
    fn api_base_derives_from_host_and_trims_slash() {
        let p = test_profile("https://app.nestr.io/");
        assert_eq!(p.api_base(), "https://app.nestr.io/api");
    }

    #[test]
    fn oauth_urls_derive_with_defaults() {
        let p = test_profile("https://app.nestr.io");
        assert_eq!(p.token_url(), "https://app.nestr.io/oauth/token");
        assert_eq!(p.authorize_url(), "https://app.nestr.io/dialog/oauth");
    }

    #[test]
    fn oauth_urls_honor_overrides() {
        let mut p = test_profile("https://app.nestr.io");
        p.oauth_token_url = Some("https://x/t".into());
        p.oauth_authorize_url = Some("https://x/a".into());
        assert_eq!(p.token_url(), "https://x/t");
        assert_eq!(p.authorize_url(), "https://x/a");
    }

    fn test_profile(host: &str) -> Profile {
        Profile {
            auth: AuthKind::ApiKey,
            credential_storage: CredentialStorage::File,
            host: host.to_string(),
            workspace_id: "ws1".to_string(),
            api_key: None,
            label: None,
            oauth_client_id: None,
            oauth_token_url: None,
            oauth_authorize_url: None,
            oauth_tokens: None,
            default_output_format: None,
        }
    }
}
