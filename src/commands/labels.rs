use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render::print_json;
use crate::views::LabelView;
use crate::{render, safety};

#[derive(Subcommand)]
pub enum LabelsCmd {
    /// List workspace labels.
    List {
        #[arg(long)]
        search: Option<String>,
    },
    /// Get one workspace label by id.
    Get { label_id: String },
    /// Create a workspace label (admin).
    Create {
        title: String,
        /// Id for the new label (e.g. review). Defaults to one derived from the title.
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        /// Label group, used to group labels in pickers.
        #[arg(long)]
        group: Option<String>,
        #[command(flatten)]
        user_opts: UserLabelOpts,
    },
    /// Update a workspace label (admin).
    Update {
        label_id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        /// Label group, used to group labels in pickers.
        #[arg(long)]
        group: Option<String>,
        #[command(flatten)]
        user_opts: UserLabelOpts,
        /// Reset the label to apply to nests only.
        #[arg(long, conflicts_with = "applies_to")]
        reset_applies_to: bool,
        /// Remove all user field definitions.
        #[arg(long, conflicts_with = "user_fields")]
        clear_user_fields: bool,
    },
    /// Delete a workspace label (admin). It comes off every item carrying it.
    Delete { label_id: String },
    /// Personal labels.
    Personal {
        #[command(subcommand)]
        cmd: PersonalCmd,
    },
}

/// Options that make a label apply to people.
#[derive(clap::Args, Default)]
pub struct UserLabelOpts {
    /// What the label applies to: nests, users, or both (e.g. --applies-to nests,users).
    #[arg(long, value_delimiter = ',', value_parser = ["nests", "users"])]
    pub applies_to: Vec<String>,
    /// User field definitions as a JSON array (or @file.json). Each item is
    /// {"id","code","title","type", "required"?, "options"?}; types: text, textarea,
    /// url, number, select, multiselect, date, checkbox. Replaces the whole list.
    #[arg(long, value_name = "JSON")]
    pub user_fields: Option<String>,
}

#[derive(Subcommand)]
pub enum PersonalCmd {
    /// List your personal labels.
    List {
        #[arg(long)]
        search: Option<String>,
    },
    /// Create a personal label.
    Create {
        title: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        icon: Option<String>,
    },
}

pub async fn fetch_workspace(
    client: &NestrClient,
    workspace_id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{workspace_id}/labels"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_workspace_one(
    client: &NestrClient,
    workspace_id: &str,
    label_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(
            &format!("/workspaces/{workspace_id}/labels/{label_id}"),
            &[],
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_workspace(
    client: &NestrClient,
    workspace_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{workspace_id}/labels"), body)
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn update_workspace(
    client: &NestrClient,
    workspace_id: &str,
    label_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(
            &format!("/workspaces/{workspace_id}/labels/{label_id}"),
            body,
        )
        .await?;
    Ok(unwrap_data(raw).0)
}

pub async fn delete_workspace(
    client: &NestrClient,
    workspace_id: &str,
    label_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/workspaces/{workspace_id}/labels/{label_id}"))
        .await?;
    Ok(unwrap_data(raw).0)
}

/// Parse `--user-fields`: a JSON array, or `@path` to a file holding one.
pub fn parse_user_fields(arg: &str) -> crate::error::Result<Value> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)?,
        None => arg.to_string(),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(v) if v.is_array() => Ok(v),
        _ => Err(crate::error::NestrError::Validation(
            "--user-fields expects a JSON array of field definitions".into(),
        )),
    }
}

/// Body for label create/update. Absent options are omitted; the reset/clear
/// flags send `null`, which the API reads as "back to default".
#[allow(clippy::too_many_arguments)]
pub fn label_body(
    title: Option<String>,
    color: Option<String>,
    icon: Option<String>,
    group: Option<String>,
    opts: &UserLabelOpts,
    reset_applies_to: bool,
    clear_user_fields: bool,
) -> crate::error::Result<Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in [
        ("title", title),
        ("color", color),
        ("icon", icon),
        ("group", group),
    ] {
        if let Some(v) = v {
            map.insert(k.into(), v.into());
        }
    }
    if reset_applies_to {
        map.insert("applicableTo".into(), Value::Null);
    } else if !opts.applies_to.is_empty() {
        map.insert("applicableTo".into(), serde_json::json!(opts.applies_to));
    }
    if clear_user_fields {
        map.insert("userFields".into(), Value::Null);
    } else if let Some(f) = &opts.user_fields {
        map.insert("userFields".into(), parse_user_fields(f)?);
    }
    Ok(Value::Object(map))
}

/// Body for label create. POST stores the body as the label itself, which keeps
/// colour and icon under `properties` (only PATCH reads them as top-level
/// shortcuts), so they move there; `id` becomes the label's `_id`.
pub fn create_body(
    title: String,
    id: Option<String>,
    color: Option<String>,
    icon: Option<String>,
    group: Option<String>,
    opts: &UserLabelOpts,
) -> crate::error::Result<Value> {
    let mut body = label_body(Some(title), None, None, group, opts, false, false)?;
    let map = body.as_object_mut().expect("label_body builds an object");
    let mut properties = serde_json::Map::new();
    for (k, v) in [("color", color), ("icon", icon)] {
        if let Some(v) = v {
            properties.insert(k.into(), v.into());
        }
    }
    if !properties.is_empty() {
        map.insert("properties".into(), Value::Object(properties));
    }
    if let Some(id) = id {
        map.insert("_id".into(), id.into());
    }
    Ok(body)
}

pub async fn fetch_personal(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me/labels", params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_personal(client: &NestrClient, body: &Value) -> crate::error::Result<Value> {
    let raw: Value = client.post("/users/me/labels", body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn render_labels(data: &Value, output: crate::config::OutputFormat) -> Result<()> {
    match output {
        crate::config::OutputFormat::Json => print_json(data)?,
        crate::config::OutputFormat::Text => {
            let labels: Vec<LabelView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if labels.is_empty() {
                render::print_no_results("No labels.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = labels
                .iter()
                .map(|l| {
                    vec![
                        l.id.clone(),
                        l.title.clone().unwrap_or_default(),
                        l.shown_color().to_string(),
                        l.shown_icon().to_string(),
                        l.applicable_to.join(","),
                    ]
                })
                .collect();
            println!(
                "{}",
                render::format_table(&["ID", "TITLE", "COLOR", "ICON", "APPLIES TO"], rows)
            );
        }
    }
    Ok(())
}

fn render_user_fields(label: &Value) {
    let Some(defs) = label["userFields"].as_array().filter(|a| !a.is_empty()) else {
        return;
    };
    println!("user fields:");
    for d in defs {
        let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("");
        println!(
            "  {}  {}  ({}){}",
            render::clean_text(s("code")),
            render::clean_text(s("title")),
            render::clean_text(s("type")),
            if d["required"].as_bool() == Some(true) {
                " required"
            } else {
                ""
            }
        );
    }
}

fn output_label(data: &Value, output: crate::config::OutputFormat) -> Result<()> {
    match output {
        crate::config::OutputFormat::Json => print_json(data)?,
        crate::config::OutputFormat::Text => {
            render_labels(&Value::Array(vec![data.clone()]), output)?;
            render_user_fields(data);
        }
    }
    Ok(())
}

pub async fn run(cmd: LabelsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        LabelsCmd::List { search } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &search {
                params.push(("search", s));
            }
            let data = fetch_workspace(&client, cfg.require_workspace()?, &params).await?;
            render_labels(&data, cfg.output)?;
        }
        LabelsCmd::Get { label_id } => {
            let data = fetch_workspace_one(&client, cfg.require_workspace()?, &label_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    render_labels(&Value::Array(vec![data.clone()]), cfg.output)?;
                    render_user_fields(&data);
                }
            }
        }
        LabelsCmd::Create {
            title,
            id,
            color,
            icon,
            group,
            user_opts,
        } => {
            safety::enforce_read_only(g.read_only, "labels create")?;
            let body = create_body(title, id, color, icon, group, &user_opts)?;
            let data = create_workspace(&client, cfg.require_workspace()?, &body).await?;
            output_label(&data, cfg.output)?;
        }
        LabelsCmd::Update {
            label_id,
            title,
            color,
            icon,
            group,
            user_opts,
            reset_applies_to,
            clear_user_fields,
        } => {
            safety::enforce_read_only(g.read_only, "labels update")?;
            let body = label_body(
                title,
                color,
                icon,
                group,
                &user_opts,
                reset_applies_to,
                clear_user_fields,
            )?;
            if body.as_object().is_some_and(|m| m.is_empty()) {
                anyhow::bail!("labels update: nothing to change, pass at least one option");
            }
            let data =
                update_workspace(&client, cfg.require_workspace()?, &label_id, &body).await?;
            output_label(&data, cfg.output)?;
        }
        LabelsCmd::Delete { label_id } => {
            safety::enforce_read_only(g.read_only, "labels delete")?;
            safety::confirm_destructive(
                &format!(
                    "Delete workspace label '{label_id}'? It comes off every item carrying it, \
                     and this cannot be undone."
                ),
                g.yes,
            )?;
            let data = delete_workspace(&client, cfg.require_workspace()?, &label_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    let id = data["_id"].as_str().unwrap_or(&label_id);
                    println!("deleted label {}", render::clean_text(id));
                }
            }
        }
        LabelsCmd::Personal { cmd } => match cmd {
            PersonalCmd::List { search } => {
                let mut params: Vec<(&str, &str)> = Vec::new();
                if let Some(s) = &search {
                    params.push(("search", s));
                }
                let data = fetch_personal(&client, &params).await?;
                render_labels(&data, cfg.output)?;
            }
            PersonalCmd::Create {
                title,
                description,
                color,
                icon,
            } => {
                safety::enforce_read_only(g.read_only, "labels personal create")?;
                let mut map = serde_json::Map::new();
                map.insert("title".into(), title.into());
                if let Some(d) = description {
                    map.insert("description".into(), d.into());
                }
                if let Some(c) = color {
                    map.insert("color".into(), c.into());
                }
                if let Some(i) = icon {
                    map.insert("icon".into(), i.into());
                }
                let data = create_personal(&client, &Value::Object(map)).await?;
                render_labels(&Value::Array(vec![data]), cfg.output)?;
            }
        },
    }
    Ok(())
}
