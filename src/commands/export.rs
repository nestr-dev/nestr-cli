use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

#[derive(Subcommand)]
pub enum ExportCmd {
    /// Dump the workspace governance tree as JSON.
    Governance,
    /// Dump the workspace work view as JSON.
    Work,
}

pub async fn fetch_governance(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/governance"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_work(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/workspaces/{ws}/work"), &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: ExportCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.workspace_id.clone();
    // Export is a JSON dump regardless of -o (the nested tree has no table form).
    let data = match cmd {
        ExportCmd::Governance => fetch_governance(&client, &ws).await?,
        ExportCmd::Work => fetch_work(&client, &ws).await?,
    };
    render::print_json(&data)?;
    Ok(())
}
