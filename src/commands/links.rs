use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render::{self, print_json};
use crate::safety;

#[derive(Subcommand)]
pub enum LinksCmd {
    /// List graph links of a nest.
    List {
        nest_id: String,
        /// Filter by relation (e.g. meeting).
        #[arg(long)]
        relation: Option<String>,
        /// in = incoming links, out = outgoing.
        #[arg(long, value_parser = ["in", "out"])]
        direction: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// Link a nest to a target (bidirectional).
    Add {
        nest_id: String,
        relation: String,
        target_id: String,
    },
    /// Remove a graph link.
    Remove {
        nest_id: String,
        relation: String,
        target_id: String,
    },
}

pub async fn fetch_links(
    client: &NestrClient,
    nest: &str,
    relation: Option<&str>,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let path = match relation {
        Some(r) => format!("/nests/{nest}/graph/{r}"),
        None => format!("/nests/{nest}/graph"),
    };
    let raw: Value = client.get(&path, params).await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn add_link(
    client: &NestrClient,
    nest: &str,
    relation: &str,
    target: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            &format!("/nests/{nest}/graph/{relation}"),
            &serde_json::json!({ "targetId": target }),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn remove_link(
    client: &NestrClient,
    nest: &str,
    relation: &str,
    target: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/nests/{nest}/graph/{relation}/{target}"))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Map the CLI `--direction in|out` to the API's `incoming`/`outgoing`.
fn map_direction(d: &str) -> &'static str {
    if d == "in" {
        "incoming"
    } else {
        "outgoing"
    }
}

pub async fn run(cmd: LinksCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        LinksCmd::List {
            nest_id,
            relation,
            direction,
            limit,
            page,
        } => {
            let limit = limit.map(|n| n.to_string());
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            let dir = direction.as_deref().map(map_direction);
            if let Some(d) = dir {
                params.push(("direction", d));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let (data, meta) = fetch_links(&client, &nest_id, relation.as_deref(), &params).await?;
            render::output_links(&data, meta.as_ref(), cfg.output)?;
        }
        LinksCmd::Add {
            nest_id,
            relation,
            target_id,
        } => {
            safety::enforce_read_only(g.read_only, "links add")?;
            let data = add_link(&client, &nest_id, &relation, &target_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    println!("Linked {target_id} via '{relation}'.")
                }
            }
        }
        LinksCmd::Remove {
            nest_id,
            relation,
            target_id,
        } => {
            safety::enforce_read_only(g.read_only, "links remove")?;
            let data = remove_link(&client, &nest_id, &relation, &target_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    let msg = data
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Graph link removed");
                    println!("{msg}");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_mapping() {
        assert_eq!(map_direction("in"), "incoming");
        assert_eq!(map_direction("out"), "outgoing");
    }
}
