use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render;

#[derive(Subcommand)]
pub enum InsightsCmd {
    /// List workspace insights (metrics) for a period, each with its value and the
    /// value it compares against. One circle (--nest) or one person (--user) needs
    /// a Pro plan.
    List {
        /// One person's insights (Pro): `me` for yourself, which needs a token that
        /// acts for you. Cannot be combined with --nest.
        #[arg(long, conflicts_with = "nest")]
        user: Option<String>,
        /// One circle's insights (Pro). Defaults to the workspace.
        #[arg(long, alias = "circle")]
        nest: Option<String>,
        /// Include sub-circles' activity (server default: true).
        #[arg(long, num_args = 0..=1, default_missing_value = "true")]
        include_sub_circles: Option<bool>,
        /// Start of the period (ISO 8601). Defaults to one month before --to. The
        /// compare period has the same length and ends here.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now.
        #[arg(long, alias = "end-date")]
        to: Option<String>,
    },
    /// Get one metric by its type id, with its value for a period.
    Get {
        metric_id: String,
        /// Start of the period (ISO 8601). Defaults to one month before --to.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now.
        #[arg(long)]
        to: Option<String>,
    },
    /// Show a metric's history: the workspace's own, one circle's (--nest, Pro) or
    /// one person's (--user, Pro). The points of a metric nest, goal or other
    /// dataseries field are under `nests series` instead.
    History {
        metric_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        /// At most this many points (server default 1000).
        #[arg(long)]
        limit: Option<u32>,
        /// One circle's series (Pro). Defaults to the workspace.
        #[arg(long, alias = "circle")]
        nest: Option<String>,
        /// One person's series (Pro): `me` for yourself. Cannot be combined with --nest.
        #[arg(long, conflicts_with = "nest")]
        user: Option<String>,
    },
    /// Compare one metric across the direct child circles of a circle (Pro). The
    /// parent is listed too, but its value includes its children, so it is set apart.
    Compare {
        metric_id: String,
        /// The circle whose child circles to compare. Defaults to the workspace.
        #[arg(long)]
        parent: Option<String>,
        /// Start of the period (ISO 8601). Defaults to one month before --to.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now; runs to the end of that day.
        #[arg(long)]
        to: Option<String>,
        /// Whether each circle's value includes its sub-circles (server default: true).
        #[arg(long, num_args = 0..=1, default_missing_value = "true")]
        include_sub_circles: Option<bool>,
    },
}

/// Who and when an insights list covers.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub user: Option<String>,
    pub nest: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub include_sub_circles: Option<bool>,
}

/// The query `GET workspaces/:id/insights` takes for a scope.
pub fn list_params(s: &Scope) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    if let Some(u) = &s.user {
        params.push(("userId", u.clone()));
    }
    if let Some(n) = &s.nest {
        params.push(("nestId", n.clone()));
    }
    if let Some(b) = s.include_sub_circles {
        params.push(("includeSubCircles", b.to_string()));
    }
    if let Some(f) = &s.from {
        params.push(("from", f.clone()));
    }
    if let Some(t) = &s.to {
        params.push(("to", t.clone()));
    }
    params
}

fn as_pairs<'a>(params: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    params.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

fn push_opt(params: &mut Vec<(&'static str, String)>, key: &'static str, v: Option<String>) {
    if let Some(v) = v {
        params.push((key, v));
    }
}

/// `GET workspaces/:id/insights`, with its meta (the periods analysed).
pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

/// `GET workspaces/:id/insights/:metric`, with its meta.
pub async fn fetch_get(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights/{metric}"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
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

/// `GET workspaces/:id/insights/:metric/circles`, with its meta.
pub async fn fetch_compare(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(
            &format!("/workspaces/{ws}/insights/{metric}/circles"),
            params,
        )
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

fn print_with_meta(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    text: impl Fn(&Value, Option<&Value>) -> String,
) -> Result<()> {
    match output {
        OutputFormat::Json => {
            render::print_json(data)?;
            render::emit_meta(meta, output);
        }
        OutputFormat::Text => println!("{}", text(data, meta)),
    }
    Ok(())
}

pub async fn run(cmd: InsightsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    let output = cfg.output;
    match cmd {
        InsightsCmd::List {
            user,
            nest,
            include_sub_circles,
            from,
            to,
        } => {
            let params = list_params(&Scope {
                user,
                nest,
                from,
                to,
                include_sub_circles,
            });
            let (data, meta) = fetch_list(&client, &ws, &as_pairs(&params)).await?;
            print_with_meta(&data, meta.as_ref(), output, render::insights_text)?;
        }
        InsightsCmd::Get {
            metric_id,
            from,
            to,
        } => {
            let mut params = Vec::new();
            push_opt(&mut params, "from", from);
            push_opt(&mut params, "to", to);
            let (data, meta) = fetch_get(&client, &ws, &metric_id, &as_pairs(&params)).await?;
            print_with_meta(&data, meta.as_ref(), output, render::insight_detail_text)?;
        }
        InsightsCmd::History {
            metric_id,
            from,
            to,
            limit,
            nest,
            user,
        } => {
            let mut params = Vec::new();
            push_opt(&mut params, "from", from);
            push_opt(&mut params, "to", to);
            push_opt(&mut params, "limit", limit.map(|n| n.to_string()));
            push_opt(&mut params, "nestId", nest);
            push_opt(&mut params, "userId", user);
            let data = fetch_history(&client, &ws, &metric_id, &as_pairs(&params)).await?;
            render::output_history(&data, output)?;
        }
        InsightsCmd::Compare {
            metric_id,
            parent,
            from,
            to,
            include_sub_circles,
        } => {
            let mut params = Vec::new();
            push_opt(&mut params, "parentNestId", parent);
            push_opt(&mut params, "from", from);
            push_opt(&mut params, "to", to);
            push_opt(
                &mut params,
                "includeSubCircles",
                include_sub_circles.map(|b| b.to_string()),
            );
            let (data, meta) = fetch_compare(&client, &ws, &metric_id, &as_pairs(&params)).await?;
            print_with_meta(&data, meta.as_ref(), output, render::compare_text)?;
        }
    }
    Ok(())
}
