use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

#[derive(Subcommand)]
pub enum ProjectsCmd {
    /// List the workspace's projects.
    List {
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        clean_text: bool,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    workspace_id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{workspace_id}/projects"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn run(cmd: ProjectsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        ProjectsCmd::List { limit, clean_text } => {
            let limit = limit.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if clean_text {
                params.push(("cleanText", "true"));
            }
            let (data, meta) = fetch_list(&client, &cfg.workspace_id, &params).await?;
            render::output_nests(&data, meta.as_ref(), cfg.output)?;
        }
    }
    Ok(())
}
