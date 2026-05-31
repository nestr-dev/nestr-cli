use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::{render, safety};

#[derive(Subcommand)]
pub enum PlanCmd {
    /// Show today's plan (nests labelled `now`).
    Today,
    /// Add nests to today's plan.
    Add { ids: Vec<String> },
    /// Remove nests from today's plan.
    Remove { ids: Vec<String> },
}

pub async fn fetch_today(client: &NestrClient) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me/today", &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Add (`add=true`) or remove the personal `now` label on one nest.
pub async fn set_now(client: &NestrClient, id: &str, add: bool) -> crate::error::Result<Value> {
    let verb = if add { "add_label" } else { "remove_label" };
    let raw: Value = client
        .patch(&format!("/nests/{id}/{verb}/now"), &serde_json::json!({}))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: PlanCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        PlanCmd::Today => {
            let data = fetch_today(&client).await?;
            render::output_nests(&data, None, cfg.output)?;
        }
        PlanCmd::Add { ids } => {
            safety::enforce_read_only(g.read_only, "plan add")?;
            for id in &ids {
                set_now(&client, id, true).await?;
            }
            println!("Added {} nest(s) to today's plan.", ids.len());
        }
        PlanCmd::Remove { ids } => {
            safety::enforce_read_only(g.read_only, "plan remove")?;
            for id in &ids {
                set_now(&client, id, false).await?;
            }
            println!("Removed {} nest(s) from today's plan.", ids.len());
        }
    }
    Ok(())
}
