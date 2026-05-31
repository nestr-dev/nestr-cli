use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;

#[derive(Subcommand)]
pub enum NestsCmd {
    /// Get one or more nests by id (comma-join multiple).
    Get {
        /// Nest id(s), comma-separated for several.
        ids: String,
        #[arg(long)]
        clean_text: bool,
        /// Include field metadata.
        #[arg(long)]
        fields_meta: bool,
        /// Include API next-step hints.
        #[arg(long)]
        hints: bool,
    },
    /// List the direct children of a nest.
    Children {
        id: String,
        /// Filter children by text (server switches to depth-1 search).
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
        #[arg(long)]
        clean_text: bool,
    },
}

pub async fn fetch_get(
    client: &NestrClient,
    ids: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/nests/{ids}"), params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_children(
    client: &NestrClient,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client.get(&format!("/nests/{id}/children"), params).await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn run(cmd: NestsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        NestsCmd::Get {
            ids,
            clean_text,
            fields_meta,
            hints,
        } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if clean_text {
                params.push(("cleanText", "true"));
            }
            if fields_meta {
                params.push(("fieldsMetaData", "true"));
            }
            if hints {
                params.push(("hints", "true"));
            }
            let data = fetch_get(&client, &ids, &params).await?;
            if data.is_array() {
                render::output_nests(&data, None, cfg.output)?;
            } else {
                render::output_nest_detail(&data, cfg.output)?;
            }
        }
        NestsCmd::Children {
            id,
            search,
            limit,
            page,
            clean_text,
        } => {
            let limit = limit.map(|n| n.to_string());
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &search {
                params.push(("search", s));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            if clean_text {
                params.push(("cleanText", "true"));
            }
            let (data, meta) = fetch_children(&client, &id, &params).await?;
            render::output_nests(&data, meta.as_ref(), cfg.output)?;
        }
    }
    Ok(())
}
