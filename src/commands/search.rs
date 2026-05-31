use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

#[derive(Args)]
pub struct SearchArgs {
    /// Text to search for.
    pub query: String,
    /// Search within a single nest subtree instead of the whole workspace.
    #[arg(long, value_name = "NEST_ID")]
    pub r#in: Option<String>,
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(long)]
    pub page: Option<u32>,
    /// Strip rich-text markup from results.
    #[arg(long)]
    pub clean_text: bool,
}

/// Run the search and return `(data, meta)`, both already unwrapped.
pub async fn run_search(
    client: &NestrClient,
    workspace_id: &str,
    query: &str,
    in_nest: Option<&str>,
    extra: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let mut params: Vec<(&str, &str)> = vec![("search", query)];
    params.extend_from_slice(extra);
    let raw: Value = match in_nest {
        Some(id) => client.get(&format!("/nests/{id}/search"), &params).await?,
        None => {
            client
                .get(&format!("/workspaces/{workspace_id}/search"), &params)
                .await?
        }
    };
    let (data, meta, _links) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn run(args: SearchArgs, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;

    // Build owned strings for any numeric/bool params, then borrow into &str pairs.
    let limit = args.limit.map(|n| n.to_string());
    let page = args.page.map(|n| n.to_string());
    let mut extra: Vec<(&str, &str)> = Vec::new();
    if let Some(l) = &limit {
        extra.push(("limit", l));
    }
    if let Some(p) = &page {
        extra.push(("page", p));
    }
    if args.clean_text {
        extra.push(("cleanText", "true"));
    }

    let (data, meta) = run_search(
        &client,
        &cfg.workspace_id,
        &args.query,
        args.r#in.as_deref(),
        &extra,
    )
    .await?;
    render::output_nests(&data, meta.as_ref(), cfg.output)?;
    Ok(())
}
