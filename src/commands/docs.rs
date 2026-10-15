//! `nestr workspaces docs`: the workspace's reference documents (constitution,
//! handbook, onboarding guide, …) uploaded for its agents. They are files on the
//! workspace nest under the `nestradamus_files` group, so this is a thin view over
//! the files routes.

use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::api_client::NestrClient;
use crate::commands::files;
use crate::config::{OutputFormat, ResolvedConfig};
use crate::render::{self, print_json};

/// The group id the workspace's context-documents field uploads under.
pub const CONTEXT: &str = "nestradamus_files";

#[derive(Subcommand)]
pub enum DocsCmd {
    /// The index: each document with a description of what it holds.
    List,
    /// Find the passages across all documents that match.
    Search {
        /// Words the document would use.
        query: String,
    },
    /// Read one document's text (a slice when large; --all for the whole of it).
    Read {
        file_id: String,
        /// Continue from a previous nextOffset, or a search hit's offset.
        #[arg(long)]
        offset: Option<u64>,
        /// Follow nextOffset to the end and print the whole document.
        #[arg(long)]
        all: bool,
    },
}

pub async fn list(
    client: &NestrClient,
    workspace_id: &str,
    search: Option<&str>,
) -> crate::error::Result<Value> {
    files::list_files(client, workspace_id, Some(CONTEXT), search).await
}

/// Read from `offset` to the end, following `nextOffset`, into one slice-shaped value.
pub async fn read_all(
    client: &NestrClient,
    workspace_id: &str,
    file_id: &str,
    offset: Option<u64>,
) -> Result<Value> {
    let mut first = files::read_file_text(client, workspace_id, file_id, offset).await?;
    let mut text = first
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut at = first
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(offset.unwrap_or(0));
    let mut next = first.get("nextOffset").and_then(Value::as_u64);
    while let Some(n) = next {
        if n <= at {
            bail!("reading {file_id}: the server's nextOffset ({n}) did not advance past {at}");
        }
        at = n;
        let slice = files::read_file_text(client, workspace_id, file_id, Some(n)).await?;
        text.push_str(slice.get("text").and_then(Value::as_str).unwrap_or(""));
        next = slice.get("nextOffset").and_then(Value::as_u64);
    }
    first["text"] = json!(text);
    first["nextOffset"] = Value::Null;
    Ok(first)
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn output_index(data: &Value, output: OutputFormat) -> Result<()> {
    if output == OutputFormat::Json {
        return print_json(data);
    }
    let docs = data.as_array().cloned().unwrap_or_default();
    if docs.is_empty() {
        render::print_no_results("This workspace has uploaded no reference documents.");
        return Ok(());
    }
    for d in &docs {
        let size = d.get("size").and_then(Value::as_u64).unwrap_or(0);
        println!(
            "{}  [{}]  {}",
            render::clean_text(str_of(d, "name")),
            render::clean_text(str_of(d, "id")),
            files::format_bytes(size)
        );
        let desc = str_of(d, "description");
        if desc.is_empty() {
            println!("  (no description yet)");
        } else {
            println!("  {}", render::clean_text(desc));
        }
    }
    Ok(())
}

pub async fn run(cmd: DocsCmd, cfg: &ResolvedConfig, client: &NestrClient) -> Result<()> {
    let ws = cfg.require_workspace()?;
    match cmd {
        DocsCmd::List => {
            let data = list(client, ws, None).await?;
            output_index(&data, cfg.output)?;
        }
        DocsCmd::Search { query } => {
            let data = list(client, ws, Some(&query)).await?;
            files::output_files(
                &data,
                cfg.output,
                &format!("No passage in this workspace's documents matches \"{query}\"."),
            )?;
            if cfg.output == OutputFormat::Text && data.as_array().is_some_and(|a| !a.is_empty()) {
                eprintln!(
                    "(read on from a hit with `nestr workspaces docs read <id> --offset <n>`)"
                );
            }
        }
        DocsCmd::Read {
            file_id,
            offset,
            all,
        } => {
            let data = if all {
                read_all(client, ws, &file_id, offset).await?
            } else {
                files::read_file_text(client, ws, &file_id, offset).await?
            };
            let more = format!("nestr workspaces docs read {file_id}");
            files::output_text_slice(&data, cfg.output, &more)?;
        }
    }
    Ok(())
}
