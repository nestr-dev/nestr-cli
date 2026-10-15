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
    #[arg(long, conflicts_with = "in")]
    pub page: Option<u32>,
    /// Strip rich-text markup from results.
    #[arg(long)]
    pub clean_text: bool,
    /// Reject the search if any operator, label or field filter is not recognised,
    /// instead of silently returning a broader result. Use it when counting.
    #[arg(long)]
    pub strict: bool,
    /// Sort order, e.g. `title`, `-createdAt` (overrides a sort: operator).
    #[arg(long)]
    pub sort: Option<String>,
}

/// Append the `strict:true` operator unless the query already carries it.
pub fn strict_query(query: &str, strict: bool) -> String {
    let has = query
        .split_whitespace()
        .any(|w| w.eq_ignore_ascii_case("strict:true"));
    if !strict || has {
        query.to_string()
    } else {
        format!("{query} strict:true")
    }
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
    if let Some(s) = &args.sort {
        extra.push(("sort", s));
    }
    let query = strict_query(&args.query, args.strict);

    // A workspace-wide search needs a workspace; a nest-scoped search (--in) does not.
    let workspace = if args.r#in.is_some() {
        ""
    } else {
        cfg.require_workspace()?
    };
    let (data, meta) = run_search(&client, workspace, &query, args.r#in.as_deref(), &extra).await?;
    render::output_nests(&data, meta.as_ref(), cfg.output, true)?;
    Ok(())
}
