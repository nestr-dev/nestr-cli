use anyhow::Result;
use clap::Subcommand;
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render;
use crate::safety;

#[derive(Subcommand)]
pub enum RolesCmd {
    /// List all roles in the workspace.
    List,
    /// Get a role by id.
    Get { id: String },
    /// Create a role inside a circle (admin).
    Create {
        #[arg(long)]
        title: String,
        /// The circle this role belongs to (required).
        #[arg(long, required = true)]
        parent: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
    },
    /// Update a role (replaces accountabilities/domains if given) (admin).
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
}

pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/roles"), params)
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
        .get(&format!("/workspaces/{ws}/roles/{id}"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_role(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/roles"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_role(
    client: &NestrClient,
    ws: &str,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/workspaces/{ws}/roles/{id}"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn clean_params(output: OutputFormat) -> Vec<(&'static str, &'static str)> {
    if output == OutputFormat::Text {
        vec![("cleanText", "true")]
    } else {
        Vec::new()
    }
}

pub async fn run(cmd: RolesCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.workspace_id.clone();
    match cmd {
        RolesCmd::List => {
            let (data, meta) = fetch_list(&client, &ws, &clean_params(cfg.output)).await?;
            render::output_roles(&data, meta.as_ref(), cfg.output)?;
        }
        RolesCmd::Get { id } => {
            let data = fetch_get(&client, &ws, &id, &clean_params(cfg.output)).await?;
            render::role_detail(&data, cfg.output)?;
        }
        RolesCmd::Create {
            title,
            parent,
            purpose,
            description,
            accountabilities,
            domains,
        } => {
            safety::enforce_read_only(g.read_only, "roles create")?;
            let mut body = Map::new();
            body.insert("title".into(), title.into());
            body.insert("parentId".into(), parent.into());
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if let Some(d) = description {
                body.insert("description".into(), d.into());
            }
            if !accountabilities.is_empty() {
                body.insert(
                    "accountabilities".into(),
                    serde_json::to_value(&accountabilities)?,
                );
            }
            if !domains.is_empty() {
                body.insert("domains".into(), serde_json::to_value(&domains)?);
            }
            let data = create_role(&client, &ws, &Value::Object(body)).await?;
            render::role_detail(&data, cfg.output)?;
        }
        RolesCmd::Update {
            id,
            title,
            purpose,
            description,
            accountabilities,
            domains,
        } => {
            safety::enforce_read_only(g.read_only, "roles update")?;
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
            if !accountabilities.is_empty() {
                body.insert(
                    "accountabilities".into(),
                    serde_json::to_value(&accountabilities)?,
                );
            }
            if !domains.is_empty() {
                body.insert("domains".into(), serde_json::to_value(&domains)?);
            }
            let data = update_role(&client, &ws, &id, &Value::Object(body)).await?;
            render::role_detail(&data, cfg.output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_params_only_in_text_mode() {
        assert_eq!(
            clean_params(OutputFormat::Text),
            vec![("cleanText", "true")]
        );
        assert!(clean_params(OutputFormat::Json).is_empty());
    }
}
