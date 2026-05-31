use std::collections::HashMap;

use anyhow::{bail, Context, Result};

const SERVICE_NAME: &str = "nestr-cli";

/// True when a real OS keyring backend is compiled into this build. When false
/// (e.g. static `musl` Linux binaries, which ship no backend), the `keyring`
/// crate silently falls back to a non-persistent in-memory mock — so `os-store`
/// credential storage would lose secrets. Keep this in sync with the
/// platform-gated `keyring` backend features in Cargo.toml.
pub fn os_keyring_available() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "musl")),
    ))
}

type SecretMap = HashMap<String, String>;

fn load_map(profile: &str) -> Result<Option<SecretMap>> {
    let entry = keyring::Entry::new(SERVICE_NAME, profile).context("creating keyring entry")?;
    match entry.get_password() {
        Ok(json) => Ok(Some(
            serde_json::from_str(&json).context("parsing keyring secrets")?,
        )),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("reading from keyring: {e}")),
    }
}

fn save_map(profile: &str, map: &SecretMap) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, profile).context("creating keyring entry")?;
    if map.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => eprintln!("Warning: failed to delete keyring entry for {profile}: {e}"),
        }
    } else {
        let json = serde_json::to_string(map).context("serializing keyring secrets")?;
        entry
            .set_password(&json)
            .context("storing secrets in keyring")?;
    }
    Ok(())
}

pub fn store_secret(profile: &str, key: &str, secret: &str) -> Result<()> {
    if !os_keyring_available() {
        bail!(
            "This build has no OS keyring backend (e.g. static musl Linux binaries ship without one). \
             Use file-based credential storage instead of 'os-store'."
        );
    }
    let mut map = load_map(profile)?.unwrap_or_default();
    map.insert(key.to_string(), secret.to_string());
    save_map(profile, &map)
}

pub fn get_secret(profile: &str, key: &str) -> Result<Option<String>> {
    Ok(load_map(profile)?.and_then(|m| m.get(key).cloned()))
}

pub fn delete_profile(profile: &str) {
    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, profile) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => eprintln!("Warning: failed to delete keyring entry for {profile}: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "musl"))
    ))]
    fn os_keyring_available_when_backend_compiled() {
        assert!(os_keyring_available());
    }

    #[test]
    #[ignore = "requires a system keyring"]
    fn roundtrip() {
        let profile = "nestr_test_roundtrip";
        store_secret(profile, "api_key", "secret-123").unwrap();
        assert_eq!(
            get_secret(profile, "api_key").unwrap(),
            Some("secret-123".into())
        );
        delete_profile(profile);
        assert_eq!(get_secret(profile, "api_key").unwrap(), None);
    }
}
