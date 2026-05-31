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
    /// Personal labels.
    Personal {
        #[command(subcommand)]
        cmd: PersonalCmd,
    },
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
                        l.color.clone().unwrap_or_default(),
                        l.icon.clone().unwrap_or_default(),
                    ]
                })
                .collect();
            println!(
                "{}",
                render::format_table(&["ID", "TITLE", "COLOR", "ICON"], rows)
            );
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
            let data = fetch_workspace(&client, &cfg.workspace_id, &params).await?;
            render_labels(&data, cfg.output)?;
        }
        LabelsCmd::Get { label_id } => {
            let data = fetch_workspace_one(&client, &cfg.workspace_id, &label_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    render_labels(&Value::Array(vec![data]), cfg.output)?
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
