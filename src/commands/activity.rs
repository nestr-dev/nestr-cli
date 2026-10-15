use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render::{self, print_json};

#[derive(Args)]
pub struct ActivityArgs {
    /// Another user's activity, across the workspaces you share (default: your own).
    #[arg(long, value_name = "USER_ID")]
    pub user: Option<String>,
    /// Your own activity only: show agent considerations from your DM with this user.
    #[arg(long, value_name = "USER_ID", conflicts_with = "user")]
    pub with_user: Option<String>,
    /// Max entries (server default 50, max 200).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=200))]
    pub limit: Option<u32>,
}

/// Activity entries, newest first. `user` None is the caller's own feed.
pub async fn fetch(
    client: &NestrClient,
    user: Option<&str>,
    limit: Option<u32>,
    with_user: Option<&str>,
) -> crate::error::Result<Value> {
    let limit = limit.map(|n| n.to_string());
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(l) = &limit {
        params.push(("limit", l));
    }
    if let Some(w) = with_user {
        params.push(("withUser", w));
    }
    let path = match user {
        Some(id) => format!("/users/{id}/activity"),
        None => "/users/me/activity".to_string(),
    };
    let raw: Value = client.get(&path, &params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data
        .get("activity")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![])))
}

/// One line saying what an entry did: the description, or for an agent
/// consideration its summary with the tools used and the outcome.
pub fn what(entry: &Value) -> String {
    let s = |k: &str| entry.get(k).and_then(Value::as_str).unwrap_or("");
    let base = if s("description").is_empty() {
        s("summary")
    } else {
        s("description")
    };
    let mut extra: Vec<String> = Vec::new();
    let tools: Vec<&str> = entry
        .get("tools")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !tools.is_empty() {
        extra.push(format!("tools: {}", tools.join(", ")));
    }
    if !s("outcome").is_empty() {
        extra.push(format!("outcome: {}", s("outcome")));
    }
    let line = if extra.is_empty() {
        base.to_string()
    } else {
        format!("{base} ({})", extra.join("; "))
    };
    render::clean_text(&line)
}

fn when(entry: &Value) -> String {
    let d = entry.get("date").and_then(Value::as_str).unwrap_or("");
    render::clean_text(&d.chars().take(16).collect::<String>().replace('T', " "))
}

pub async fn run(args: ActivityArgs, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let data = fetch(
        &client,
        args.user.as_deref(),
        args.limit,
        args.with_user.as_deref(),
    )
    .await?;
    if cfg.output == OutputFormat::Json {
        return print_json(&data);
    }
    let entries = data.as_array().cloned().unwrap_or_default();
    if entries.is_empty() {
        render::print_no_results("No activity.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|e| {
            let s = |k: &str| render::clean_text(e.get(k).and_then(Value::as_str).unwrap_or(""));
            vec![when(e), s("type"), s("nestTitle"), s("nestId"), what(e)]
        })
        .collect();
    println!(
        "{}",
        render::format_table(&["DATE", "TYPE", "NEST", "NEST ID", "WHAT"], rows)
    );
    Ok(())
}
