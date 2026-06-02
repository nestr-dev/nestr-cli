use anyhow::Result;
use clap::Subcommand;
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render::{self, print_json};
use crate::views::PostView;
use crate::{safety, validation};

#[derive(Subcommand)]
pub enum CirclesCmd {
    /// List circles in the workspace.
    List,
    /// Get a circle by id.
    Get { id: String },
    /// Create a circle (admin).
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Parent circle id (for a nested circle).
        #[arg(long)]
        parent: Option<String>,
        /// An accountability (repeatable).
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        /// A domain (repeatable).
        #[arg(long = "domain")]
        domains: Vec<String>,
        /// Extra label (repeatable).
        #[arg(long = "label")]
        labels: Vec<String>,
    },
    /// Update a circle (replaces accountabilities/domains if given) (admin).
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
    },
    /// List a circle's roles.
    Roles { id: String },
    /// List a circle's projects.
    Projects { id: String },
    /// List a circle's posts (comments).
    Posts {
        id: String,
        #[arg(long)]
        depth: Option<String>,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/circles"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_get(
    client: &NestrClient,
    ws: &str,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/circles/{id}"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_circle(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/circles"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_circle(
    client: &NestrClient,
    ws: &str,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/circles/{id}"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_roles(
    client: &NestrClient,
    ws: &str,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/circles/{id}/roles"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_projects(
    client: &NestrClient,
    ws: &str,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/circles/{id}/projects"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_posts(
    client: &NestrClient,
    ws: &str,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/circles/{id}/posts"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Insert accountabilities/domains string arrays into a body map if non-empty.
fn put_acc_domains(body: &mut Map<String, Value>, acc: &[String], dom: &[String]) -> Result<()> {
    if !acc.is_empty() {
        body.insert("accountabilities".into(), serde_json::to_value(acc)?);
    }
    if !dom.is_empty() {
        body.insert("domains".into(), serde_json::to_value(dom)?);
    }
    Ok(())
}

fn render_posts(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let posts: Vec<PostView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if posts.is_empty() {
                render::print_no_results("No posts.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = posts
                .iter()
                .map(|p| {
                    vec![
                        p.id.clone(),
                        p.created_by.clone().unwrap_or_default(),
                        p.text(),
                    ]
                })
                .collect();
            println!("{}", render::format_table(&["ID", "BY", "BODY"], rows));
        }
    }
    Ok(())
}

/// `cleanText=true` for text mode (accountability titles are HTML); raw for JSON.
fn clean_params(output: OutputFormat) -> Vec<(&'static str, &'static str)> {
    if output == OutputFormat::Text {
        vec![("cleanText", "true")]
    } else {
        Vec::new()
    }
}

pub async fn run(cmd: CirclesCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.workspace_id.clone();
    match cmd {
        CirclesCmd::List => {
            let (data, meta) = fetch_list(&client, &ws, &clean_params(cfg.output)).await?;
            render::output_roles(&data, meta.as_ref(), cfg.output)?;
        }
        CirclesCmd::Get { id } => {
            let data = fetch_get(&client, &ws, &id, &clean_params(cfg.output)).await?;
            render::role_detail(&data, cfg.output)?;
        }
        CirclesCmd::Create {
            title,
            purpose,
            description,
            parent,
            accountabilities,
            domains,
            labels,
        } => {
            safety::enforce_read_only(g.read_only, "circles create")?;
            if !labels.is_empty() {
                validation::validate_prime_labels(&labels)?;
            }
            let mut body = Map::new();
            body.insert("title".into(), title.into());
            if let Some(p) = parent {
                body.insert("parentId".into(), p.into());
            }
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if let Some(d) = description {
                body.insert("description".into(), d.into());
            }
            if !labels.is_empty() {
                body.insert("labels".into(), serde_json::to_value(&labels)?);
            }
            put_acc_domains(&mut body, &accountabilities, &domains)?;
            let data = create_circle(&client, &ws, &Value::Object(body)).await?;
            render::role_detail(&data, cfg.output)?;
        }
        CirclesCmd::Update {
            id,
            title,
            purpose,
            description,
            accountabilities,
            domains,
        } => {
            safety::enforce_read_only(g.read_only, "circles update")?;
            let mut body = Map::new();
            if let Some(t) = title {
                body.insert("title".into(), t.into());
            }
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if let Some(d) = description {
                body.insert("description".into(), d.into());
            }
            put_acc_domains(&mut body, &accountabilities, &domains)?;
            let data = update_circle(&client, &ws, &id, &Value::Object(body)).await?;
            render::role_detail(&data, cfg.output)?;
        }
        CirclesCmd::Roles { id } => {
            let (data, meta) = fetch_roles(&client, &ws, &id, &clean_params(cfg.output)).await?;
            render::output_roles(&data, meta.as_ref(), cfg.output)?;
        }
        CirclesCmd::Projects { id } => {
            let (data, meta) = fetch_projects(&client, &ws, &id, &[]).await?;
            render::output_nests(&data, meta.as_ref(), cfg.output)?;
        }
        CirclesCmd::Posts { id, depth } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(d) = &depth {
                params.push(("depth", d));
            }
            let data = fetch_posts(&client, &ws, &id, &params).await?;
            render_posts(&data, cfg.output)?;
        }
    }
    Ok(())
}
