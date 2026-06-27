use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

#[derive(Subcommand)]
pub enum InsightsCmd {
    /// List workspace insights (metrics). Filters by user/circle need a Pro plan.
    List {
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        circle: Option<String>,
        #[arg(long)]
        include_sub_circles: Option<bool>,
        #[arg(long)]
        end_date: Option<String>,
    },
    /// Get one metric by its type id.
    Get { metric_id: String },
    /// Show a metric's history.
    History {
        metric_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_get(
    client: &NestrClient,
    ws: &str,
    metric: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights/{metric}"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_history(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(
            &format!("/workspaces/{ws}/insights/{metric}/history"),
            params,
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: InsightsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    match cmd {
        InsightsCmd::List {
            user,
            circle,
            include_sub_circles,
            end_date,
        } => {
            let isc = include_sub_circles.map(|b| b.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(u) = &user {
                params.push(("userId", u));
            }
            if let Some(c) = &circle {
                params.push(("nestId", c));
            }
            if let Some(s) = &isc {
                params.push(("includeSubCircles", s));
            }
            if let Some(e) = &end_date {
                params.push(("endDate", e));
            }
            let data = fetch_list(&client, &ws, &params).await?;
            render::output_insights(&data, cfg.output)?;
        }
        InsightsCmd::Get { metric_id } => {
            let data = fetch_get(&client, &ws, &metric_id).await?;
            render::insight_detail(&data, cfg.output)?;
        }
        InsightsCmd::History {
            metric_id,
            from,
            to,
            limit,
        } => {
            let limit = limit.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(f) = &from {
                params.push(("from", f));
            }
            if let Some(t) = &to {
                params.push(("to", t));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            let data = fetch_history(&client, &ws, &metric_id, &params).await?;
            render::output_history(&data, cfg.output)?;
        }
    }
    Ok(())
}
