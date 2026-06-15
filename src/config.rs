use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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

pub fn load_config() -> Result<Config> {
    let path = config_file();
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&raw).context("parsing config.toml")
}

pub fn save_config(config: &Config) -> Result<()> {
    std::fs::create_dir_all(config_dir())?;
    let path = config_file();
    let content = toml::to_string_pretty(config).context("serializing config")?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn load_profile(name: &str) -> Result<Profile> {
    validate_profile_name(name)?;
    let path = profile_file(name);
    let raw = std::fs::read_to_string(&path).with_context(|| {
        format!("Profile '{name}' not found. Run `nestr profiles add` to set it up.")
    })?;
    toml::from_str(&raw).with_context(|| format!("parsing profile '{name}'"))
}

/// Reject profile names that could escape the profiles directory or create stray
/// files. Allowed: ASCII alphanumerics plus `_`, `-`, `.` — no path separators,
/// no `..`. (COR-8)
pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        anyhow::bail!("invalid profile name '{name}'");
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        anyhow::bail!("invalid profile name '{name}': use letters, digits, '_', '-', '.' only");
    }
    Ok(())
}

/// Create a directory (recursively) with mode 0700 on unix. (SEC-9)
fn create_dir_secure(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("creating {}", dir.display()))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    Ok(())
}

/// Atomically write `bytes` to `path` with mode 0600 (unix): write a temp file in
/// the same directory created 0600, then rename over the target. Secrets never
/// exist world-readable and a crash can't leave a half-written/permissive file.
/// (SEC-8)
fn write_secure_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().expect("profile path has a parent");
    let stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("profile");
    let tmp = dir.join(format!(".{stem}.{}.tmp", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    // Don't leak the temp file if write/sync/rename fails partway.
    let write_res: std::io::Result<()> = (|| {
        f.write_all(bytes)?;
        f.sync_all()
    })();
    drop(f);
    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!("writing {}", tmp.display())));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!(
            "renaming {} -> {}",
            tmp.display(),
            path.display()
        )));
    }
    Ok(())
}

pub fn save_profile(name: &str, profile: &Profile) -> Result<()> {
    validate_profile_name(name)?;
    create_dir_secure(&profiles_dir())?;
    let path = profile_file(name);
    let content = toml::to_string_pretty(profile).context("serializing profile")?;
    write_secure_atomic(&path, content.as_bytes())?;
    Ok(())
}

/// Hold an exclusive advisory lock on the profiles dir for the duration of a
/// read-modify-write of profile tokens, so two concurrent `nestr` processes
/// (scripted/agent use) can't interleave a refresh. Released when the returned
/// File drops. Uses `std::fs::File::lock` (stable since Rust 1.89). (SEC-10)
fn lock_profiles() -> Result<std::fs::File> {
    create_dir_secure(&profiles_dir())?;
    let f = std::fs::File::create(profiles_dir().join(".lock")).context("opening profiles lock")?;
    f.lock().context("locking profiles dir")?;
    Ok(f)
}

/// Update just the OAuth token set on a stored profile. Takes an exclusive lock,
/// then reloads from disk; if another process already wrote a still-valid token,
/// keeps that instead of clobbering it with ours.
pub fn update_profile_oauth_tokens(
    name: &str,
    tokens: &crate::oauth::StoredOAuthTokens,
) -> Result<()> {
    validate_profile_name(name)?;
    let _lock = lock_profiles()?;
    let mut profile = load_profile(name)?;
    if let Some(existing) = &profile.oauth_tokens {
        if existing.is_valid() && existing != tokens {
            return Ok(());
        }
    }
    profile.oauth_tokens = Some(tokens.clone());
    save_profile(name, &profile)
}

pub fn delete_profile_file(name: &str) -> Result<()> {
    validate_profile_name(name)?;
    let path = profile_file(name);
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

pub fn list_profile_names() -> Result<Vec<String>> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? != "toml" {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_string())
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Bearer precedence: explicit flag > env var > profile value.
pub fn resolve_bearer_precedence(
    flag: Option<&str>,
    env: Option<&str>,
    profile_val: Option<&str>,
) -> Option<String> {
    flag.or(env).or(profile_val).map(|s| s.to_string())
}

/// Everything a command needs to talk to Nestr for one profile.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub profile_name: String,
    pub bearer: String,
    pub host: String,
    pub api_base: String,
    pub workspace_id: String,
    pub output: OutputFormat,
    pub refresh: Option<crate::oauth::ReactiveRefresh>,
}

/// Resolve the active profile into a ready-to-use config.
/// `--api-key` / `NESTR_API_KEY` / `--host` overrides are applied.
pub async fn resolve(
    profile_override: Option<&str>,
    api_key_flag: Option<&str>,
    host_flag: Option<&str>,
    output_flag: Option<OutputFormat>,
) -> Result<ResolvedConfig> {
    let global = load_config().unwrap_or_default();
    let env_profile = std::env::var("NESTR_PROFILE").ok();
    let name = profile_override
        .map(str::to_string)
        .or(env_profile)
        .unwrap_or_else(|| global.default_profile.clone());

    let mut profile = load_profile(&name)?;
    if let Some(h) = host_flag {
        profile.host = h.to_string();
    }

    let env_api_key = std::env::var("NESTR_API_KEY").ok();
    let bearer = match profile.auth {
        AuthKind::ApiKey => {
            let profile_key = match profile.credential_storage {
                CredentialStorage::OsStore => crate::keyring_store::get_secret(&name, "api_key")?,
                CredentialStorage::File => profile.api_key.clone(),
            };
            resolve_bearer_precedence(api_key_flag, env_api_key.as_deref(), profile_key.as_deref())
                .ok_or_else(|| {
                    anyhow::anyhow!("No API key for profile '{name}'. Run `nestr profiles add`.")
                })?
        }
        AuthKind::OAuth => {
            // A flag/env API key still overrides OAuth if explicitly provided.
            if let Some(k) = resolve_bearer_precedence(api_key_flag, env_api_key.as_deref(), None) {
                k
            } else {
                let (bearer, refreshed) = crate::oauth::resolve_token(
                    &name,
                    &profile.token_url(),
                    &profile.client_id(),
                    profile.credential_storage,
                    profile.oauth_tokens.as_ref(),
                )
                .await?;
                if let Some(new_tokens) = refreshed {
                    profile.oauth_tokens = Some(new_tokens);
                    save_profile(&name, &profile)?;
                }
                bearer
            }
        }
    };

    let output = output_flag
        .or(profile.default_output_format)
        .unwrap_or(global.default_output_format);

    // Build the reactive-refresh context for OAuth profiles that still hold a
    // refresh token (so a 403 can refresh-and-retry once). API-key profiles: None.
    let refresh = if profile.auth == AuthKind::OAuth {
        crate::oauth::current_refresh_token(
            &name,
            profile.credential_storage,
            profile.oauth_tokens.as_ref(),
        )?
        .map(|rt| {
            crate::oauth::ReactiveRefresh::new(
                profile.token_url(),
                profile.client_id(),
                name.clone(),
                profile.credential_storage,
                rt,
            )
        })
    } else {
        None
    };

    Ok(ResolvedConfig {
        profile_name: name,
        bearer,
        host: profile.host.clone(),
        api_base: profile.api_base(),
        workspace_id: profile.workspace_id.clone(),
        output,
        refresh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialize tests that mutate the NESTR_HOME env var so they don't race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn update_profile_oauth_tokens_roundtrips() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("NESTR_HOME", tmp.path());
        let mut p = test_profile("https://app.nestr.io");
        p.auth = AuthKind::OAuth;
        save_profile("oauthp", &p).unwrap();

        let toks = crate::oauth::StoredOAuthTokens {
            access_token: "acc".into(),
            refresh_token: Some("ref".into()),
            id_token: None,
            expiry: Some(crate::oauth::unix_now_secs() + 99),
        };
        update_profile_oauth_tokens("oauthp", &toks).unwrap();
        let loaded = load_profile("oauthp").unwrap();
        assert_eq!(loaded.oauth_tokens.unwrap().access_token, "acc");
        std::env::remove_var("NESTR_HOME");
    }

    #[test]
    fn precedence_flag_beats_env_beats_profile() {
        assert_eq!(
            resolve_bearer_precedence(Some("flag"), Some("env"), Some("prof")),
            Some("flag".to_string())
        );
        assert_eq!(
            resolve_bearer_precedence(None, Some("env"), Some("prof")),
            Some("env".to_string())
        );
        assert_eq!(
            resolve_bearer_precedence(None, None, Some("prof")),
            Some("prof".to_string())
        );
        assert_eq!(resolve_bearer_precedence(None, None, None), None);
    }

    #[test]
    fn rejects_profile_names_with_traversal() {
        assert!(validate_profile_name("prod").is_ok());
        assert!(validate_profile_name("local-2.dev").is_ok());
        assert!(validate_profile_name("../evil").is_err());
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name("").is_err());
    }

    #[test]
    fn save_profile_rejects_traversal_name() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("NESTR_HOME", tmp.path());
        let p = test_profile("https://app.nestr.io");
        assert!(save_profile("../escape", &p).is_err());
        std::env::remove_var("NESTR_HOME");
    }

    #[test]
    #[cfg(unix)]
    fn profiles_dir_is_0700_after_save() {
        use std::os::unix::fs::PermissionsExt;
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("NESTR_HOME", tmp.path());
        let p = test_profile("https://app.nestr.io");
        save_profile("p", &p).unwrap();
        let mode = std::fs::metadata(profiles_dir())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        std::env::remove_var("NESTR_HOME");
    }

    #[test]
    fn save_then_load_profile_roundtrips_and_is_0600() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("NESTR_HOME", tmp.path());

        let p = test_profile("https://app.nestr.io");
        save_profile("prod", &p).unwrap();

        let loaded = load_profile("prod").unwrap();
        assert_eq!(loaded.host, "https://app.nestr.io");
        assert_eq!(loaded.workspace_id, "ws1");
        assert_eq!(list_profile_names().unwrap(), vec!["prod".to_string()]);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(profile_file("prod"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::env::remove_var("NESTR_HOME");
    }
}
