use anyhow::{bail, Result};

use crate::api_client::NestrClient;
use crate::config::{self, AuthKind, CredentialStorage};
use crate::{keyring_store, oauth};

/// `nestr auth login [profile]` — (re)run the browser PKCE flow for a profile.
pub async fn run_login(profile: Option<String>) -> Result<()> {
    let global = config::load_config().unwrap_or_default();
    let name = profile.unwrap_or(global.default_profile);
    let mut p = config::load_profile(&name)?;
    if p.auth != AuthKind::OAuth {
        bail!("Profile '{name}' uses API-key auth. Use `nestr profiles add` to switch to OAuth.");
    }
    let tokens = oauth::browser_login(&p.authorize_url(), &p.token_url(), &p.client_id()).await?;
    match p.credential_storage {
        CredentialStorage::OsStore => {
            keyring_store::delete_profile(&name);
            oauth::store_tokens_keyring(&name, &tokens)?;
        }
        CredentialStorage::File => {
            p.oauth_tokens = Some(oauth::tokens_to_stored(&tokens));
            config::save_profile(&name, &p)?;
        }
    }
    println!("Logged in to profile '{name}'.");
    Ok(())
}

/// `nestr auth logout [profile]` — invalidate server-side and clear local creds.
pub async fn run_logout(profile: Option<String>, yes: bool) -> Result<()> {
    let global = config::load_config().unwrap_or_default();
    let name = profile.unwrap_or(global.default_profile);
    crate::safety::confirm_destructive(&format!("Log out profile '{name}'?"), yes)?;

    // Best-effort server-side invalidation.
    if let Ok(cfg) = config::resolve(Some(&name), None, None, None).await {
        if let Ok(client) = NestrClient::new(cfg.api_base, &cfg.bearer) {
            let _ = client
                .get::<serde_json::Value>("/users/me/logout", &[])
                .await;
        }
    }

    keyring_store::delete_profile(&name);
    if let Ok(mut p) = config::load_profile(&name) {
        if p.oauth_tokens.is_some() || p.api_key.is_some() {
            p.oauth_tokens = None;
            p.api_key = None;
            config::save_profile(&name, &p)?;
        }
    }
    println!("Logged out of profile '{name}'.");
    Ok(())
}

/// `nestr auth status [profile]` — show the resolved profile + token validity.
pub async fn run_status(profile: Option<String>) -> Result<()> {
    let global = config::load_config().unwrap_or_default();
    let name = profile.unwrap_or(global.default_profile);
    let p = config::load_profile(&name)?;
    println!("profile:   {name}");
    println!("host:      {}", p.host);
    println!("api_base:  {}", p.api_base());
    println!("workspace: {}", p.workspace_id);
    match p.auth {
        AuthKind::ApiKey => println!("auth:      api-key ({:?})", p.credential_storage),
        AuthKind::OAuth => {
            let tokens = match p.credential_storage {
                CredentialStorage::File => p.oauth_tokens.clone(),
                CredentialStorage::OsStore => keyring_store::get_secret(&name, "oauth_tokens")?
                    .and_then(|j| serde_json::from_str(&j).ok()),
            };
            match tokens {
                Some(t) if t.is_valid() => println!("auth:      oauth (token valid)"),
                Some(_) => println!("auth:      oauth (token expired — `nestr auth login` or it will refresh on next call)"),
                None => println!("auth:      oauth (not logged in — run `nestr auth login`)"),
            }
        }
    }
    Ok(())
}
