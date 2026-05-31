use anyhow::{bail, Result};
use inquire::{Confirm, Password, PasswordDisplayMode, Select, Text};

use crate::config::{self, AuthKind, CredentialStorage, OutputFormat, Profile};
use crate::{keyring_store, oauth};

const AUTH_METHODS: &[&str] = &["OAuth (browser login)", "API key (paste)"];
const STORAGE: &[&str] = &["file", "os-store"];

fn pick_storage() -> Result<CredentialStorage> {
    let choice = Select::new("Where should credentials be stored?", STORAGE.to_vec())
        .with_help_message("'file' = profile TOML (0600). 'os-store' = OS keyring.")
        .prompt()?;
    Ok(match choice {
        "os-store" => CredentialStorage::OsStore,
        _ => CredentialStorage::File,
    })
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
            p.label.unwrap_or_else(|| "-".into()),
            p.host,
            p.workspace_id,
            auth.into(),
            default.into(),
        ]);
    }
    println!(
        "{}",
        crate::render::format_table(
            &["NAME", "LABEL", "HOST", "WORKSPACE", "AUTH", "DEFAULT"],
            rows,
        )
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

pub async fn run_add(name: Option<String>) -> Result<()> {
    let first = config::list_profile_names()?.is_empty();
    let name = match name {
        Some(n) if !n.is_empty() => n,
        _ => {
            let mut t = Text::new("Profile name:").with_help_message("e.g. prod, staging, local");
            if first {
                t = t.with_default("default");
            }
            t.prompt()?
        }
    };

    let host = Text::new("Nestr host:")
        .with_default("https://app.nestr.io")
        .prompt()?
        .trim_end_matches('/')
        .to_string();
    let workspace_id = Text::new("Workspace ID:").prompt()?;
    let label = Text::new("Label (optional):")
        .prompt_skippable()?
        .filter(|s| !s.is_empty());

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
        host: host.clone(),
        workspace_id,
        api_key: None,
        label,
        oauth_client_id: None,
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: None,
        default_output_format: Some(OutputFormat::Text),
    };

    if use_oauth {
        keyring_store::delete_profile(&name);
        let tokens = oauth::browser_login(
            &profile.authorize_url(),
            &profile.token_url(),
            &profile.client_id(),
        )
        .await?;
        println!("Login successful.");
        profile.credential_storage = pick_storage()?;
        match profile.credential_storage {
            CredentialStorage::OsStore => oauth::store_tokens_keyring(&name, &tokens)?,
            CredentialStorage::File => {
                profile.oauth_tokens = Some(oauth::tokens_to_stored(&tokens))
            }
        }
    } else {
        let key = Password::new("Nestr API key:")
            .with_display_mode(PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()?;
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
