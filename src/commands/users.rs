use anyhow::Result;
use clap::Subcommand;
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render::{self, print_json};
use crate::safety;
use crate::views::UserView;

#[derive(Subcommand)]
pub enum UsersCmd {
    /// List users in the workspace.
    List {
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        include_suspended: bool,
    },
    /// Get a user by id.
    Get { id: String },
    /// Add a user to the workspace by email (admin).
    Add {
        email: String,
        #[arg(long)]
        full_name: Option<String>,
        #[arg(long)]
        language: Option<String>,
    },
    /// Update a user (admin).
    Update {
        id: String,
        #[arg(long)]
        full_name: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        username: Option<String>,
    },
    /// List the roles a user fills.
    Roles { id: String },
    /// Show or set a user's group membership.
    Groups {
        #[command(subcommand)]
        cmd: UserGroupsCmd,
    },
}

#[derive(Subcommand)]
pub enum UserGroupsCmd {
    /// Show a user's groups.
    Show { id: String },
    /// Replace a user's groups (admin).
    Set {
        id: String,
        #[arg(required = true, num_args = 1..)]
        names: Vec<String>,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/users"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_get(client: &NestrClient, ws: &str, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/users/{id}"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn add_user(client: &NestrClient, ws: &str, body: &Value) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/users"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_user(
    client: &NestrClient,
    ws: &str,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/users/{id}"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_user_roles(
    client: &NestrClient,
    ws: &str,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/users/{id}/roles"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_user_groups(
    client: &NestrClient,
    ws: &str,
    id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/users/{id}/groups"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn set_user_groups(
    client: &NestrClient,
    ws: &str,
    id: &str,
    names: &[String],
) -> crate::error::Result<Value> {
    let body = serde_json::to_value(names)?;
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/users/{id}/groups"), &body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn render_users(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let users: Vec<UserView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if users.is_empty() {
                render::print_no_results("No users.");
            } else {
                println!("{}", render::user_table(&users));
            }
        }
    }
    Ok(())
}

fn render_group_names(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let names: Vec<String> = serde_json::from_value(data.clone()).unwrap_or_default();
            if names.is_empty() {
                render::print_no_results("No groups.");
            } else {
                for n in names {
                    println!("{}", render::clean_text(&n));
                }
            }
        }
    }
    Ok(())
}

pub async fn run(cmd: UsersCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.workspace_id.clone();
    match cmd {
        UsersCmd::List {
            search,
            include_suspended,
        } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &search {
                params.push(("search", s));
            }
            if include_suspended {
                params.push(("includeSuspended", "true"));
            }
            let (data, _) = fetch_list(&client, &ws, &params).await?;
            render_users(&data, cfg.output)?;
        }
        UsersCmd::Get { id } => {
            let data = fetch_get(&client, &ws, &id).await?;
            render_users(&Value::Array(vec![data]), cfg.output)?;
        }
        UsersCmd::Add {
            email,
            full_name,
            language,
        } => {
            safety::enforce_read_only(g.read_only, "users add")?;
            let mut profile = Map::new();
            if let Some(n) = full_name {
                profile.insert("fullName".into(), n.into());
            }
            if let Some(l) = language {
                profile.insert("language".into(), l.into());
            }
            let mut body = Map::new();
            body.insert("username".into(), email.into());
            if !profile.is_empty() {
                body.insert("profile".into(), Value::Object(profile));
            }
            let data = add_user(&client, &ws, &Value::Object(body)).await?;
            render_users(&Value::Array(vec![data]), cfg.output)?;
        }
        UsersCmd::Update {
            id,
            full_name,
            email,
            username,
        } => {
            safety::enforce_read_only(g.read_only, "users update")?;
            let mut profile = Map::new();
            if let Some(n) = full_name {
                profile.insert("fullName".into(), n.into());
            }
            if let Some(e) = email {
                profile.insert("email".into(), e.into());
            }
            let mut body = Map::new();
            if let Some(u) = username {
                body.insert("username".into(), u.into());
            }
            if !profile.is_empty() {
                body.insert("profile".into(), Value::Object(profile));
            }
            let data = update_user(&client, &ws, &id, &Value::Object(body)).await?;
            render_users(&Value::Array(vec![data]), cfg.output)?;
        }
        UsersCmd::Roles { id } => {
            let (data, meta) = fetch_user_roles(&client, &ws, &id, &[]).await?;
            render::output_roles(&data, meta.as_ref(), cfg.output, false)?;
        }
        UsersCmd::Groups { cmd } => match cmd {
            UserGroupsCmd::Show { id } => {
                let data = fetch_user_groups(&client, &ws, &id).await?;
                render_group_names(&data, cfg.output)?;
            }
            UserGroupsCmd::Set { id, names } => {
                safety::enforce_read_only(g.read_only, "users groups set")?;
                let data = set_user_groups(&client, &ws, &id, &names).await?;
                render_group_names(&data, cfg.output)?;
            }
        },
    }
    Ok(())
}
