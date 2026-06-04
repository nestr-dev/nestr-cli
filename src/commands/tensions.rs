use anyhow::Result;
use clap::Subcommand;
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::{OutputFormat, ResolvedConfig};
use crate::{render, safety};

#[derive(Subcommand)]
pub enum TensionsCmd {
    /// List tensions created by or assigned to you.
    Mine {
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// List tensions awaiting your consent vote.
    AwaitingConsent {
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// List tensions under a circle/role.
    List {
        nest_id: String,
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// Get one tension.
    Get { nest_id: String, tension_id: String },
    /// Create a tension under a circle/role.
    Create {
        nest_id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        feeling: Option<String>,
        #[arg(long)]
        needs: Option<String>,
    },
    /// Update a tension.
    Update {
        nest_id: String,
        tension_id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        feeling: Option<String>,
        #[arg(long)]
        needs: Option<String>,
    },
    /// Delete a tension.
    Delete { nest_id: String, tension_id: String },
    /// Show a tension's status + vote tally.
    Status { nest_id: String, tension_id: String },
    /// Submit a draft tension for consent (draft → proposed).
    Submit { nest_id: String, tension_id: String },
    /// Retract a proposed tension (proposed → draft).
    Retract { nest_id: String, tension_id: String },
    /// Record your consent vote.
    Vote {
        nest_id: String,
        tension_id: String,
        #[arg(value_parser = ["accept", "escalate"])]
        decision: String,
    },
    /// Proposal parts of a tension.
    Parts {
        #[command(subcommand)]
        cmd: PartsCmd,
    },
}

#[derive(Subcommand)]
pub enum PartsCmd {
    /// List proposal parts.
    List { nest_id: String, tension_id: String },
    /// Propose a new role/circle/policy.
    Add {
        nest_id: String,
        tension_id: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_parser = ["role", "circle", "policy"])]
        label: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
        #[arg(long = "user")]
        users: Vec<String>,
        #[arg(long)]
        due: Option<String>,
    },
    /// Edit a proposal part you added.
    Modify {
        nest_id: String,
        tension_id: String,
        part_id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
        #[arg(long = "user")]
        users: Vec<String>,
        #[arg(long)]
        due: Option<String>,
    },
    /// Remove a proposal part you added.
    Remove {
        nest_id: String,
        tension_id: String,
        part_id: String,
    },
    /// Propose a change to an existing governance item.
    ProposeUpdate {
        nest_id: String,
        tension_id: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long = "accountability")]
        accountabilities: Vec<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
        #[arg(long = "user")]
        users: Vec<String>,
        #[arg(long)]
        due: Option<String>,
    },
    /// Propose removing an existing governance item.
    ProposeDelete {
        nest_id: String,
        tension_id: String,
        #[arg(long)]
        id: String,
    },
    /// Show the computed diff for a proposal part.
    Changes {
        nest_id: String,
        tension_id: String,
        part_id: String,
    },
    /// Accountabilities/domains of a proposal part.
    Children {
        #[command(subcommand)]
        cmd: ChildrenCmd,
    },
}

#[derive(Subcommand)]
pub enum ChildrenCmd {
    /// List a part's children.
    List {
        nest_id: String,
        tension_id: String,
        part_id: String,
    },
    /// Add an accountability/domain to a proposal part.
    Add {
        nest_id: String,
        tension_id: String,
        part_id: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_parser = ["accountability", "domain"])]
        label: Option<String>,
    },
    /// Rename a child.
    Update {
        nest_id: String,
        tension_id: String,
        part_id: String,
        child_id: String,
        #[arg(long)]
        title: String,
    },
    /// Delete a child.
    Delete {
        nest_id: String,
        tension_id: String,
        part_id: String,
        child_id: String,
    },
}

// ---- API helpers (all return unwrapped data) ----

fn base(nest: &str, tid: &str) -> String {
    format!("/nests/{nest}/tensions/{tid}")
}

pub async fn fetch_mine(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client.get("/users/me/tensions", params).await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_awaiting(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get("/users/me/tensions/awaiting-my-consent", params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_list(
    client: &NestrClient,
    nest: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/nests/{nest}/tensions"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_get(
    client: &NestrClient,
    nest: &str,
    tid: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get(&base(nest, tid), params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_tension(
    client: &NestrClient,
    nest: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/nests/{nest}/tensions"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_tension(
    client: &NestrClient,
    nest: &str,
    tid: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client.patch(&base(nest, tid), body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn delete_tension(
    client: &NestrClient,
    nest: &str,
    tid: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client.delete(&base(nest, tid)).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_status(
    client: &NestrClient,
    nest: &str,
    tid: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("{}/status", base(nest, tid)), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn set_status(
    client: &NestrClient,
    nest: &str,
    tid: &str,
    state: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(
            &format!("{}/status", base(nest, tid)),
            &serde_json::json!({ "status": state }),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Build a tension write body with feeling/needs folded into `fields`.
fn tension_body(
    title: Option<String>,
    description: Option<String>,
    feeling: Option<String>,
    needs: Option<String>,
) -> Value {
    let mut fields = Map::new();
    if let Some(f) = feeling {
        fields.insert("tension.feeling".into(), f.into());
    }
    if let Some(n) = needs {
        fields.insert("tension.needs".into(), n.into());
    }
    let mut body = Map::new();
    if let Some(t) = title {
        body.insert("title".into(), t.into());
    }
    if let Some(d) = description {
        body.insert("description".into(), d.into());
    }
    if !fields.is_empty() {
        body.insert("fields".into(), Value::Object(fields));
    }
    Value::Object(body)
}

fn clean_params(output: OutputFormat) -> Vec<(&'static str, &'static str)> {
    if output == OutputFormat::Text {
        vec![("cleanText", "true")]
    } else {
        Vec::new()
    }
}

pub async fn run(cmd: TensionsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        TensionsCmd::Mine { context, page } => {
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(c) = &context {
                params.push(("context", c));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let (data, meta) = fetch_mine(&client, &params).await?;
            render::output_tensions(&data, meta.as_ref(), cfg.output)?;
        }
        TensionsCmd::AwaitingConsent { context, page } => {
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(c) = &context {
                params.push(("context", c));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let (data, meta) = fetch_awaiting(&client, &params).await?;
            render::output_tensions(&data, meta.as_ref(), cfg.output)?;
        }
        TensionsCmd::List {
            nest_id,
            search,
            limit,
            page,
        } => {
            let limit = limit.map(|n| n.to_string());
            let page = page.map(|n| n.to_string());
            let mut params = clean_params(cfg.output);
            if let Some(s) = &search {
                params.push(("search", s));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let (data, meta) = fetch_list(&client, &nest_id, &params).await?;
            render::output_tensions(&data, meta.as_ref(), cfg.output)?;
        }
        TensionsCmd::Get {
            nest_id,
            tension_id,
        } => {
            let data = fetch_get(&client, &nest_id, &tension_id, &clean_params(cfg.output)).await?;
            render::tension_detail(&data, cfg.output)?;
        }
        TensionsCmd::Create {
            nest_id,
            title,
            description,
            feeling,
            needs,
        } => {
            safety::enforce_read_only(g.read_only, "tensions create")?;
            let body = tension_body(Some(title), description, feeling, needs);
            let data = create_tension(&client, &nest_id, &body).await?;
            render::tension_detail(&data, cfg.output)?;
        }
        TensionsCmd::Update {
            nest_id,
            tension_id,
            title,
            description,
            feeling,
            needs,
        } => {
            safety::enforce_read_only(g.read_only, "tensions update")?;
            let body = tension_body(title, description, feeling, needs);
            let data = update_tension(&client, &nest_id, &tension_id, &body).await?;
            render::tension_detail(&data, cfg.output)?;
        }
        TensionsCmd::Delete {
            nest_id,
            tension_id,
        } => {
            safety::enforce_read_only(g.read_only, "tensions delete")?;
            safety::confirm_destructive(&format!("Delete tension '{tension_id}'?"), g.yes)?;
            let data = delete_tension(&client, &nest_id, &tension_id).await?;
            let msg = data
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("deleted");
            println!("{msg} ({tension_id})");
        }
        TensionsCmd::Status {
            nest_id,
            tension_id,
        } => {
            let data = fetch_status(&client, &nest_id, &tension_id).await?;
            render::output_status(&data, cfg.output)?;
        }
        TensionsCmd::Submit {
            nest_id,
            tension_id,
        } => {
            safety::enforce_read_only(g.read_only, "tensions submit")?;
            let data = set_status(&client, &nest_id, &tension_id, "proposed").await?;
            render::output_status(&data, cfg.output)?;
        }
        TensionsCmd::Retract {
            nest_id,
            tension_id,
        } => {
            safety::enforce_read_only(g.read_only, "tensions retract")?;
            let data = set_status(&client, &nest_id, &tension_id, "draft").await?;
            render::output_status(&data, cfg.output)?;
        }
        TensionsCmd::Vote {
            nest_id,
            tension_id,
            decision,
        } => {
            safety::enforce_read_only(g.read_only, "tensions vote")?;
            let state = if decision == "accept" {
                "accepted"
            } else {
                "escalated"
            };
            let data = set_status(&client, &nest_id, &tension_id, state).await?;
            render::output_status(&data, cfg.output)?;
        }
        TensionsCmd::Parts { cmd } => run_parts(cmd, &cfg, &client, g).await?,
    }
    Ok(())
}

// Parts dispatch is implemented in Task 4; stub returns until then.
async fn run_parts(
    _cmd: PartsCmd,
    _cfg: &ResolvedConfig,
    _client: &NestrClient,
    _g: &GlobalArgs,
) -> Result<()> {
    anyhow::bail!("parts commands are implemented in the next task")
}
