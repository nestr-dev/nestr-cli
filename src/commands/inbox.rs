use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::{render, safety};

#[derive(Subcommand)]
pub enum InboxCmd {
    /// List inbox items (non-completed by default).
    List {
        /// Also include items completed on/after this ISO date.
        #[arg(long)]
        completed_after: Option<String>,
    },
    /// Get one inbox item.
    Get {
        id: String,
        #[arg(long)]
        clean_text: bool,
    },
    /// Capture a new inbox item.
    Create {
        title: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update an inbox item.
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        completed: Option<bool>,
    },
    /// Reorder inbox items to the given order.
    Reorder {
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
}

pub async fn fetch_list(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client.get("/users/me/inbox", params).await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn fetch_get(
    client: &NestrClient,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/users/me/inbox/{id}"), params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn create_item(
    client: &NestrClient,
    title: &str,
    description: Option<&str>,
) -> crate::error::Result<Value> {
    let mut map = serde_json::Map::new();
    map.insert("title".into(), title.into());
    if let Some(d) = description {
        map.insert("description".into(), d.into());
    }
    let raw: Value = client.post("/users/me/inbox", &Value::Object(map)).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_item(
    client: &NestrClient,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client.patch(&format!("/users/me/inbox/{id}"), body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn reorder(client: &NestrClient, ids: &[String]) -> crate::error::Result<Value> {
    let body = serde_json::to_value(ids)?;
    let raw: Value = client.patch("/users/me/inbox/reorder", &body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: InboxCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        InboxCmd::List { completed_after } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(c) = &completed_after {
                params.push(("completedAfter", c));
            }
            let (data, meta) = fetch_list(&client, &params).await?;
            render::output_nests(&data, meta.as_ref(), cfg.output)?;
        }
        InboxCmd::Get { id, clean_text } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if clean_text {
                params.push(("cleanText", "true"));
            }
            let data = fetch_get(&client, &id, &params).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        InboxCmd::Create { title, description } => {
            safety::enforce_read_only(g.read_only, "inbox create")?;
            let data = create_item(&client, &title, description.as_deref()).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        InboxCmd::Update {
            id,
            title,
            description,
            completed,
        } => {
            safety::enforce_read_only(g.read_only, "inbox update")?;
            let mut map = serde_json::Map::new();
            if let Some(t) = title {
                map.insert("title".into(), t.into());
            }
            if let Some(d) = description {
                map.insert("description".into(), d.into());
            }
            if let Some(c) = completed {
                map.insert("completed".into(), c.into());
            }
            let data = update_item(&client, &id, &Value::Object(map)).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        InboxCmd::Reorder { ids } => {
            safety::enforce_read_only(g.read_only, "inbox reorder")?;
            let data = reorder(&client, &ids).await?;
            render::output_nests(&data, None, cfg.output)?;
        }
    }
    Ok(())
}
