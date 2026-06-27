use anyhow::{bail, Result};
use inquire::{Confirm, Password, PasswordDisplayMode, Select, Text};

use crate::api_client::NestrClient;
use crate::commands::workspaces;
use crate::config::{self, AuthKind, CredentialStorage, OutputFormat, Profile};
use crate::{keyring_store, oauth};

const AUTH_METHODS: &[&str] = &["OAuth (browser login)", "API key (paste)"];
const STORAGE: &[&str] = &["file", "os-store"];

fn pick_storage() -> Result<CredentialStorage> {
    let choice = Select::new("Where should credentials be stored?", STORAGE.to_vec())
        .with_help_message("'file' = profile TOML (0600). 'os-store' = OS keyring.")
        .prompt()?;
    match choice {
        "os-store" if !keyring_store::os_keyring_available() => bail!(
            "This build has no OS keyring backend (e.g. static musl Linux binaries ship without one), \
             so 'os-store' would silently lose credentials. Choose 'file' storage instead."
        ),
        "os-store" => Ok(CredentialStorage::OsStore),
        _ => Ok(CredentialStorage::File),
    }
}

pub fn run_list() -> Result<()> {
    let global = config::load_config().unwrap_or_default();
    let names = config::list_profile_names()?;
    if names.is_empty() {
        println!("No profiles. Run `nestr profiles add`.");
        return Ok(());
    }
    let mut rows = Vec::new();
    for name in names {
        let p = config::load_profile(&name)?;
        let default = if name == global.default_profile {
            "yes"
        } else {
            ""
        };
        let auth = match p.auth {
            AuthKind::OAuth => "oauth",
            AuthKind::ApiKey => "api-key",
        };
        rows.push(vec![
            name,
            p.host,
            p.workspace_id,
            auth.into(),
            default.into(),
        ]);
    }
    println!(
        "{}",
        crate::render::format_table(&["NAME", "HOST", "WORKSPACE", "AUTH", "DEFAULT"], rows,)
    );
    Ok(())
}

pub fn run_use(name: String) -> Result<()> {
    if !config::profile_file(&name).exists() {
        bail!("Profile '{name}' not found.");
    }
    let mut global = config::load_config().unwrap_or_default();
    global.default_profile = name.clone();
    config::save_config(&global)?;
    println!("Default profile set to '{name}'.");
    Ok(())
}

pub fn run_remove(name: String, yes: bool) -> Result<()> {
    if !config::profile_file(&name).exists() {
        bail!("Profile '{name}' not found.");
    }
    crate::safety::confirm_destructive(
        &format!("Delete profile '{name}' and its credentials?"),
        yes,
    )?;
    keyring_store::delete_profile(&name);
    config::delete_profile_file(&name)?;
    println!("Profile '{name}' deleted.");
    Ok(())
}

pub async fn run_add(name: Option<String>, host_override: Option<String>) -> Result<()> {
    let first = config::list_profile_names()?.is_empty();
    let name = match name {
        Some(n) if !n.is_empty() => n,
        _ => {
            let mut t = Text::new("Profile name:").with_help_message(
                "a short name for this workspace/account — e.g. acme, client-x, side-project",
            );
            if first {
                t = t.with_default("default");
            }
            t.prompt()?
        }
    };

    // Nestr is hosted, so the host isn't prompted — it defaults to app.nestr.io.
    // Self-hosted/staging/dev users point at their instance with --host / NESTR_HOST.
    let host = host_override
        .unwrap_or_else(|| "https://app.nestr.io".to_string())
        .trim_end_matches('/')
        .to_string();

    let use_oauth = Select::new("Authentication method:", AUTH_METHODS.to_vec())
        .prompt()?
        .starts_with("OAuth");

    let mut profile = Profile {
        auth: if use_oauth {
            AuthKind::OAuth
        } else {
            AuthKind::ApiKey
        },
        credential_storage: CredentialStorage::File,
        host,
        workspace_id: String::new(), // set below from the OAuth scope or a picker
        api_key: None,
        oauth_client_id: None,
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: None,
        default_output_format: Some(OutputFormat::Text),
    };

    if use_oauth {
        let tokens = oauth::browser_login(
            &profile.authorize_url(),
            &profile.token_url(),
            &profile.client_id(),
        )
        .await?;
        println!("Login successful.");
        // The consent screen already chose the scope, so don't ask again. A
        // single-workspace grant (`nest:{id}`) pins it; a full-account grant leaves
        // the profile workspace-less — auto-selecting only when the account has
        // exactly one workspace — and you switch later with `nestr workspaces use`.
        profile.workspace_id = match nest_scope_workspace(tokens.scope.as_deref()) {
            Some(id) => id,
            None => {
                let client = NestrClient::new(profile.api_base(), &tokens.access_token)?;
                sole_workspace(&client).await?.unwrap_or_default()
            }
        };
        profile.credential_storage = pick_storage()?;
        match profile.credential_storage {
            CredentialStorage::OsStore => {
                keyring_store::delete_profile(&name);
                oauth::store_tokens_keyring(&name, &tokens)?;
            }
            CredentialStorage::File => {
                profile.oauth_tokens = Some(oauth::tokens_to_stored(&tokens))
            }
        }
    } else {
        let key = Password::new("Nestr API key:")
            .with_display_mode(PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()?;
        profile.workspace_id = Text::new("Workspace ID:").prompt()?;
        profile.credential_storage = pick_storage()?;
        match profile.credential_storage {
            CredentialStorage::OsStore => keyring_store::store_secret(&name, "api_key", &key)?,
            CredentialStorage::File => profile.api_key = Some(key),
        }
    }

    config::save_profile(&name, &profile)?;

    let mut global = config::load_config().unwrap_or_default();
    let set_default = first
        || Confirm::new(&format!("Set '{name}' as the default profile?"))
            .with_default(false)
            .prompt()?;
    if set_default {
        global.default_profile = name.clone();
        config::save_config(&global)?;
    }
    println!(
        "Profile '{name}' saved to {}.",
        config::config_dir().display()
    );
    Ok(())
}

/// Parse `nest:{workspaceId}` out of a granted OAuth scope string. The consent
/// screen sets a `nest:{id}` scope when a single workspace is chosen.
fn nest_scope_workspace(scope: Option<&str>) -> Option<String> {
    scope?
        .split_whitespace()
        .find_map(|s| s.strip_prefix("nest:").map(str::to_string))
}

/// Return the account's only workspace id, or `None` if it has zero or many.
/// A full-account profile starts workspace-less; its active workspace is chosen
/// later with `nestr workspaces use <id>` (or per command with `--workspace`).
async fn sole_workspace(client: &NestrClient) -> Result<Option<String>> {
    let (data, _) = workspaces::fetch_list(client, &[]).await?;
    let ids: Vec<String> = data
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|w| w.get("_id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    Ok(match ids.len() {
        1 => Some(ids.into_iter().next().unwrap()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nest_scope_workspace_extracts_nest_id() {
        // Single workspace chosen → token carries user: + nest:{id}.
        assert_eq!(
            nest_scope_workspace(Some("user:u1 nest:ws9")),
            Some("ws9".to_string())
        );
        assert_eq!(
            nest_scope_workspace(Some("nest:abc")),
            Some("abc".to_string())
        );
        // Full account (user-only) or no scope → no single workspace.
        assert_eq!(nest_scope_workspace(Some("user:u1")), None);
        assert_eq!(nest_scope_workspace(Some("")), None);
        assert_eq!(nest_scope_workspace(None), None);
    }
}
