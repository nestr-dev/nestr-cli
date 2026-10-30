use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::error::NestrError;
use crate::render::{self, print_json};
use crate::safety;

const DEPENDS_ON: &str = "depends_on";

/// Graph links are directed: a link runs from a source nest to a target, and the
/// target sees it as incoming. The order of the arguments is the direction.
#[derive(Subcommand)]
pub enum LinksCmd {
    /// List the graph links of a nest, with each link's direction.
    List {
        nest_id: String,
        /// Filter by relation (e.g. meeting).
        #[arg(long)]
        relation: Option<String>,
        /// in = links other nests made to this one, out = links this nest made.
        #[arg(long, value_parser = ["in", "out"])]
        direction: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// Add a directed link from SOURCE_ID to TARGET_ID.
    ///
    /// The link runs from the source to the target; the target lists it as incoming.
    /// For a dependency, the source is the dependent and the target its predecessor:
    /// `nestr links add <dependent> depends_on <predecessor>`. A dependency that would
    /// create a cycle is refused. For other relations, a link that already runs the
    /// other way is refused rather than doubled; remove it first to reverse it.
    ///
    /// Links the server writes itself are refused, and the error names what to set
    /// instead: a link field on the source (a tension's meeting, a story's sprint, epic
    /// or milestone: `nests update <id> --field tension.meeting=<meetingId>`),
    /// `proxyAssign` (`nests update <id> --proxy-assign <roleId>`), `recurrence` and
    /// `templateCopy`. `links list` still reads them.
    Add {
        source_id: String,
        relation: String,
        target_id: String,
    },
    /// Remove the link from SOURCE_ID to TARGET_ID.
    ///
    /// Only the link in that direction is removed. If the link runs the other way,
    /// the error names the command that removes it. The links `links add` refuses are
    /// refused here too: clear the field or property that made them instead.
    Remove {
        source_id: String,
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
    source: &str,
    relation: &str,
    target: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            &format!("/nests/{source}/graph/{relation}"),
            &serde_json::json!({ "targetId": target }),
        )
        .await
        .map_err(|e| explain_add_refusal(e, source, relation, target))?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn remove_link(
    client: &NestrClient,
    source: &str,
    relation: &str,
    target: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/nests/{source}/graph/{relation}/{target}"))
        .await
        .map_err(|e| explain_reverse_link(e, source, relation, target))?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// A 422 on add means the link was refused for its direction: a `depends_on` that
/// would close a cycle, or a link that already runs from the target to the source.
/// The API names the reason; add what to do about it in CLI terms.
fn explain_add_refusal(e: NestrError, source: &str, relation: &str, target: &str) -> NestrError {
    let NestrError::Validation(msg) = e else {
        return e;
    };
    let refused_dependency =
        msg.contains("would create a cycle") || msg.contains("cannot depend on each other");
    let advice = if relation == DEPENDS_ON && refused_dependency {
        format!(
            "{source} depends_on {target} means {source} waits for {target}. \
             Check which nest is the dependent, and `nestr links list {target} --relation depends_on` \
             for the existing chain."
        )
    } else if msg.contains("already links to") {
        format!(
            "Links are directed. To reverse it, run `nestr links remove {target} {relation} {source}` first."
        )
    } else {
        return NestrError::Validation(msg);
    };
    NestrError::Validation(format!("{msg}\n{advice}"))
}

/// DELETE only removes the link in the direction asked for. When the link exists
/// the other way round, the API's 404 says so; say it with the CLI command instead
/// of the HTTP call.
fn explain_reverse_link(e: NestrError, source: &str, relation: &str, target: &str) -> NestrError {
    match e {
        NestrError::NotFound(msg) if msg.contains("runs the other way") => {
            NestrError::NotFound(format!(
                "no '{relation}' link from {source} to {target}; the link runs the other way. \
                 Remove it with `nestr links remove {target} {relation} {source}`."
            ))
        }
        other => other,
    }
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
            render::output_links(&data, meta.as_ref(), cfg.output, true)?;
        }
        LinksCmd::Add {
            source_id,
            relation,
            target_id,
        } => {
            safety::enforce_read_only(g.read_only, "links add")?;
            let data = add_link(&client, &source_id, &relation, &target_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    let existed = data
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|l| l.get("status"))
                        .and_then(Value::as_str)
                        == Some("already_exists");
                    let verb = if existed { "Already linked" } else { "Linked" };
                    println!(
                        "{verb} {} -> {} ({}).",
                        render::clean_text(&source_id),
                        render::clean_text(&target_id),
                        render::clean_text(&relation)
                    );
                }
            }
        }
        LinksCmd::Remove {
            source_id,
            relation,
            target_id,
        } => {
            safety::enforce_read_only(g.read_only, "links remove")?;
            let data = remove_link(&client, &source_id, &relation, &target_id).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    let msg = data
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Graph link removed");
                    println!("{}", render::clean_text(msg));
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
