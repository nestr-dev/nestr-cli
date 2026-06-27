use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render::{self, print_json};
use crate::safety;
use crate::views::GroupView;

#[derive(Subcommand)]
pub enum GroupsCmd {
    /// List groups in the workspace.
    List,
    /// Get a group by id or name.
    Get { id: String },
    /// Create a group (admin).
    Create { name: String },
}

pub async fn fetch_list(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/workspaces/{ws}/groups"), &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_get(client: &NestrClient, ws: &str, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/groups/{id}"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_group(
    client: &NestrClient,
    ws: &str,
    name: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            &format!("/workspaces/{ws}/groups"),
            &serde_json::json!({"name": name}),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn render_groups(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let groups: Vec<GroupView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if groups.is_empty() {
                render::print_no_results("No groups.");
            } else {
                println!("{}", render::group_table(&groups));
            }
        }
    }
    Ok(())
}

pub async fn run(cmd: GroupsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    match cmd {
        GroupsCmd::List => {
            let data = fetch_list(&client, &ws).await?;
            render_groups(&data, cfg.output)?;
        }
        GroupsCmd::Get { id } => {
            let data = fetch_get(&client, &ws, &id).await?;
            render_groups(&Value::Array(vec![data]), cfg.output)?;
        }
        GroupsCmd::Create { name } => {
            safety::enforce_read_only(g.read_only, "groups create")?;
            let data = create_group(&client, &ws, &name).await?;
            render_groups(&Value::Array(vec![data]), cfg.output)?;
        }
    }
    Ok(())
}
