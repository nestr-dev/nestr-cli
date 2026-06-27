use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::{self, OutputFormat};
use crate::render::{self, print_json};
use crate::safety;
use crate::views::AppView;

#[derive(Subcommand)]
pub enum WorkspacesCmd {
    /// List your workspaces.
    List {
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// Get a workspace by id.
    Get { id: String },
    /// Create a workspace (requires a user-scoped token).
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long, value_parser = ["holacracy", "sociocracy", "roles_circles"])]
        governance: Option<String>,
        #[arg(long, value_parser = ["starter", "pro"])]
        plan: Option<String>,
        #[arg(long, value_parser = ["collaborate", "personal"])]
        collaborators: Option<String>,
        /// Enable an app on creation (repeatable): okr | feedback | insights.
        #[arg(long = "app")]
        apps: Vec<String>,
    },
    /// Show or toggle the active workspace's apps.
    Apps {
        #[command(subcommand)]
        cmd: Option<AppsCmd>,
    },
    /// Pin the active workspace for this profile (handy for full-account profiles).
    Use {
        /// Workspace id (see `nestr workspaces list`).
        id: String,
    },
}

#[derive(Subcommand)]
pub enum AppsCmd {
    /// Enable or disable an app (admin).
    Set {
        app_id: String,
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client.get("/workspaces", params).await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_get(client: &NestrClient, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/workspaces/{id}"), &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_ws(client: &NestrClient, body: &Value) -> crate::error::Result<Value> {
    let raw: Value = client.post("/workspaces", body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_apps(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/workspaces/{ws}/apps"), &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn set_apps(client: &NestrClient, ws: &str, body: &Value) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/apps"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Read-modify-write a single app's `enabled` flag: fetch all apps, flip the target,
/// PATCH the full array (safe whether the server merges or replaces).
pub async fn set_app(
    client: &NestrClient,
    ws: &str,
    app_id: &str,
    enable: bool,
) -> crate::error::Result<Value> {
    let apps = fetch_apps(client, ws).await?;
    let arr = apps.as_array().cloned().unwrap_or_default();
    let mut found = false;
    let body: Vec<Value> = arr
        .iter()
        .map(|a| {
            let id = a.get("_id").and_then(|v| v.as_str()).unwrap_or_default();
            let en = if id == app_id {
                found = true;
                enable
            } else {
                a.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false)
            };
            serde_json::json!({"_id": id, "enabled": en})
        })
        .collect();
    if !found {
        return Err(crate::error::NestrError::NotFound(format!(
            "no app '{app_id}' in this workspace (run `nestr workspaces apps` to list them)"
        )));
    }
    set_apps(client, ws, &Value::Array(body)).await
}

fn render_apps(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let apps: Vec<AppView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if apps.is_empty() {
                render::print_no_results("No apps.");
            } else {
                println!("{}", render::app_table(&apps));
            }
        }
    }
    Ok(())
}

pub async fn run(cmd: WorkspacesCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        WorkspacesCmd::List {
            search,
            limit,
            page,
        } => {
            let limit = limit.map(|n| n.to_string());
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &search {
                params.push(("search", s));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let (data, meta) = fetch_list(&client, &params).await?;
            render::output_nests(&data, meta.as_ref(), cfg.output, true)?;
        }
        WorkspacesCmd::Get { id } => {
            let data = fetch_get(&client, &id).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        WorkspacesCmd::Create {
            title,
            purpose,
            governance,
            plan,
            collaborators,
            apps,
        } => {
            safety::enforce_read_only(g.read_only, "workspaces create")?;
            let mut config = serde_json::Map::new();
            if let Some(v) = governance {
                config.insert("governance".into(), v.into());
            }
            if let Some(v) = plan {
                config.insert("plan".into(), v.into());
            }
            if let Some(v) = collaborators {
                config.insert("collaborators".into(), v.into());
            }
            if !apps.is_empty() {
                config.insert("apps".into(), serde_json::to_value(&apps)?);
            }
            let mut body = serde_json::Map::new();
            body.insert("title".into(), title.into());
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if !config.is_empty() {
                body.insert("configuration".into(), Value::Object(config));
            }
            let data = create_ws(&client, &Value::Object(body)).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        WorkspacesCmd::Apps { cmd } => match cmd {
            None => {
                let data = fetch_apps(&client, cfg.require_workspace()?).await?;
                render_apps(&data, cfg.output)?;
            }
            Some(AppsCmd::Set { app_id, state }) => {
                safety::enforce_read_only(g.read_only, "workspaces apps set")?;
                let enable = state == "on";
                let data = set_app(&client, cfg.require_workspace()?, &app_id, enable).await?;
                render_apps(&data, cfg.output)?;
            }
        },
        WorkspacesCmd::Use { id } => {
            // Validate the id is one this account can see, then pin it to the profile.
            let (data, _) = fetch_list(&client, &[]).await?;
            let known = data.as_array().is_some_and(|a| {
                a.iter()
                    .any(|w| w.get("_id").and_then(|v| v.as_str()) == Some(id.as_str()))
            });
            if !known {
                bail!(
                    "workspace '{id}' is not one of your workspaces (run `nestr workspaces list`)."
                );
            }
            let mut profile = config::load_profile(&cfg.profile_name)?;
            profile.workspace_id = id.clone();
            config::save_profile(&cfg.profile_name, &profile)?;
            println!(
                "Active workspace for profile '{}' set to {id}.",
                cfg.profile_name
            );
        }
    }
    Ok(())
}
