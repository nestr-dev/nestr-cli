use anyhow::Result;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render::{self, print_json};
use crate::views::CompactNest;

pub async fn fetch_work(client: &NestrClient, workspace_id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{workspace_id}/work"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn section(data: &Value, key: &str, heading: &str) {
    let items: Vec<CompactNest> = data
        .get(key)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    println!("\n{heading} ({})", items.len());
    if items.is_empty() {
        render::print_no_results("  (none)");
    } else {
        println!("{}", render::nest_table(&items));
    }
}

pub async fn run(g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let data = fetch_work(&client, cfg.require_workspace()?).await?;
    match cfg.output {
        crate::config::OutputFormat::Json => print_json(&data)?,
        crate::config::OutputFormat::Text => {
            section(&data, "projects", "Projects");
            section(&data, "todos", "Todos");
            if let Some(ts) = data.get("lastUpdateAt").and_then(|v| v.as_str()) {
                println!("\nlast update: {}", render::clean_text(ts));
            }
        }
    }
    Ok(())
}
