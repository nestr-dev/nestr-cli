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
        /// Only members carrying this label (short id, e.g. legal_rep). Needs a Nestr
        /// server with user labels (rolling out).
        #[arg(long, value_name = "LABEL_ID")]
        label: Option<String>,
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
    /// Put labels on people and set their user fields. Needs a Nestr server with
    /// user labels (rolling out).
    Labels {
        #[command(subcommand)]
        cmd: UserLabelsCmd,
    },
}

#[derive(Subcommand)]
pub enum UserLabelsCmd {
    /// Put a label on a member (admin by default), optionally with field values.
    Add {
        user_id: String,
        label_id: String,
        /// Field value as CODE=VALUE (repeatable), e.g. --field url=https://example.com/r/1.
        /// CODE may be a field code, id or label.code. VALUE is parsed as JSON when valid.
        #[arg(long = "field")]
        fields: Vec<String>,
    },
    /// Take a label off a member. The values are kept, hidden.
    Remove { user_id: String, label_id: String },
    /// Set field values on a label the member holds. `--field code=` clears a value.
    Set {
        user_id: String,
        label_id: String,
        #[arg(long = "field", required = true)]
        fields: Vec<String>,
    },
    /// Audit trail of label changes on a member (workspace admins), newest first.
    Activity {
        user_id: String,
        /// Max entries (server default 50, max 100).
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: Option<u32>,
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

/// Check that a `users list --label` reply was filtered by the label. A server with
/// user labels puts `labels` (short ids) on every member and returns only those
/// carrying the label; an older one ignores the param and returns everyone, with no
/// `labels`. Such a list is refused rather than shown as the label's members.
pub fn confirm_label_filter(data: &Value, label: &str, ws: &str) -> crate::error::Result<()> {
    // The label may be given by its full id, `<ws>` plus a separator and the short id.
    let matches = |short: &str| {
        short == label
            || (label.len() == ws.len() + 1 + short.len()
                && label.starts_with(ws)
                && label.ends_with(short))
    };
    let carries = |row: &Value| {
        row.get("labels")
            .and_then(Value::as_array)
            .is_some_and(|ls| ls.iter().filter_map(Value::as_str).any(matches))
    };
    let rows = data.as_array().map(Vec::as_slice).unwrap_or_default();
    if rows.iter().all(carries) {
        return Ok(());
    }
    Err(crate::error::NestrError::Ignored(
        "this Nestr server ignores --label on `nestr users list`, so it returned members \
         without that label. They were not printed. --label needs a newer Nestr server \
         (with user labels, rolling out)."
            .into(),
    ))
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

fn user_label_path(ws: &str, user_id: &str, label_id: &str) -> String {
    format!("/workspaces/{ws}/users/{user_id}/labels/{label_id}")
}

/// Body for add/set: `{"fields": {...}}`, or None when no fields were given.
pub fn fields_body(fields: &Map<String, Value>) -> Option<Value> {
    (!fields.is_empty()).then(|| serde_json::json!({ "fields": fields }))
}

pub async fn add_user_label(
    client: &NestrClient,
    ws: &str,
    user_id: &str,
    label_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&user_label_path(ws, user_id, label_id), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn set_user_label_fields(
    client: &NestrClient,
    ws: &str,
    user_id: &str,
    label_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&user_label_path(ws, user_id, label_id), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn remove_user_label(
    client: &NestrClient,
    ws: &str,
    user_id: &str,
    label_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&user_label_path(ws, user_id, label_id))
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn fetch_label_activity(
    client: &NestrClient,
    ws: &str,
    user_id: &str,
    limit: Option<u32>,
) -> crate::error::Result<Value> {
    let limit = limit.map(|n| n.to_string());
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(l) = &limit {
        params.push(("limit", l));
    }
    let raw: Value = client
        .get(
            &format!("/workspaces/{ws}/users/{user_id}/labels/activity"),
            &params,
        )
        .await?;
    Ok(unwrap_data(raw).0)
}

fn render_fields(fields: &Map<String, Value>) {
    for (k, v) in fields {
        let shown = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        println!(
            "  {} = {}",
            render::clean_text(k),
            render::clean_text(&shown)
        );
    }
}

fn render_label_change(data: &Value, verb: &str, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let s = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or("-");
            let state = if data["changed"].as_bool() == Some(false) {
                "no change"
            } else {
                verb
            };
            println!(
                "{} label {} on user {}",
                state,
                render::clean_text(s("label")),
                render::clean_text(s("userId"))
            );
            if let Some(f) = data["fields"].as_object() {
                render_fields(f);
            }
        }
    }
    Ok(())
}

fn render_label_activity(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let rows: Vec<Vec<String>> = data
                .as_array()
                .map(|a| a.as_slice())
                .unwrap_or_default()
                .iter()
                .map(|e| {
                    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    vec![s("date"), s("verb"), s("label"), s("actor")]
                })
                .collect();
            if rows.is_empty() {
                render::print_no_results("No label activity.");
            } else {
                println!(
                    "{}",
                    render::format_table(&["DATE", "VERB", "LABEL", "ACTOR"], rows)
                );
            }
        }
    }
    Ok(())
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
                // Field values are per user; show them under a single-user view.
                if let [u] = users.as_slice() {
                    render_fields(&u.fields);
                }
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
    let ws = cfg.require_workspace()?.to_string();
    match cmd {
        UsersCmd::List {
            search,
            include_suspended,
            label,
        } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &search {
                params.push(("search", s));
            }
            if include_suspended {
                params.push(("includeSuspended", "true"));
            }
            if let Some(l) = &label {
                params.push(("label", l));
            }
            let (data, _) = fetch_list(&client, &ws, &params).await?;
            if let Some(l) = &label {
                confirm_label_filter(&data, l, &ws)?;
            }
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
        UsersCmd::Labels { cmd } => match cmd {
            UserLabelsCmd::Add {
                user_id,
                label_id,
                fields,
            } => {
                safety::enforce_read_only(g.read_only, "users labels add")?;
                let parsed = super::nests::parse_field_args(&fields)?;
                let body = fields_body(&parsed).unwrap_or_else(|| serde_json::json!({}));
                let data = add_user_label(&client, &ws, &user_id, &label_id, &body).await?;
                render_label_change(&data, "added", cfg.output)?;
            }
            UserLabelsCmd::Remove { user_id, label_id } => {
                safety::enforce_read_only(g.read_only, "users labels remove")?;
                safety::confirm_destructive(
                    &format!("Remove label '{label_id}' from user '{user_id}'?"),
                    g.yes,
                )?;
                let data = remove_user_label(&client, &ws, &user_id, &label_id).await?;
                render_label_change(&data, "removed", cfg.output)?;
            }
            UserLabelsCmd::Set {
                user_id,
                label_id,
                fields,
            } => {
                safety::enforce_read_only(g.read_only, "users labels set")?;
                let parsed = super::nests::parse_field_args(&fields)?;
                let body = fields_body(&parsed).unwrap_or_default();
                let data = set_user_label_fields(&client, &ws, &user_id, &label_id, &body).await?;
                render_label_change(&data, "updated", cfg.output)?;
            }
            UserLabelsCmd::Activity { user_id, limit } => {
                let data = fetch_label_activity(&client, &ws, &user_id, limit).await?;
                render_label_activity(&data, cfg.output)?;
            }
        },
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
