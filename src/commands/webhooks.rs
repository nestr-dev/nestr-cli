use anyhow::Result;
use clap::Subcommand;
use serde_json::{Map, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render;
use crate::safety;

#[derive(Subcommand)]
pub enum WebhooksCmd {
    /// List the workspace's webhooks.
    List,
    /// Get a webhook by id.
    Get { id: String },
    /// Create a webhook (admin).
    Create {
        #[arg(long)]
        url: String,
        #[arg(long = "type", value_parser = ["nest", "comment"])]
        type_: String,
        #[arg(long, value_parser = ["create", "update", "delete"])]
        event: String,
        /// Only fire for items with this label.
        #[arg(long)]
        label: Option<String>,
        /// Only fire for events under this nest.
        #[arg(long)]
        ancestor: Option<String>,
    },
    /// Delete a webhook (admin).
    Delete { id: String },
}

pub async fn fetch_list(client: &NestrClient, ws: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/webhooks"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_get(client: &NestrClient, ws: &str, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/webhooks/{id}"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    // The route does `.fetch()` on findOne, so `data` can be a single-element array.
    let data = match data.as_array() {
        Some(arr) => arr.first().cloned().unwrap_or(Value::Null),
        None => data,
    };
    Ok(data)
}

pub async fn create_webhook(
    client: &NestrClient,
    ws: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/workspaces/{ws}/webhooks"), body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// DELETE returns a bare `"success"` string, so this returns the raw body text.
pub async fn delete_webhook(
    client: &NestrClient,
    ws: &str,
    id: &str,
) -> crate::error::Result<String> {
    client
        .delete_text(&format!("/workspaces/{ws}/webhooks/{id}"))
        .await
}

pub async fn run(cmd: WebhooksCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    match cmd {
        WebhooksCmd::List => {
            let data = fetch_list(&client, &ws).await?;
            render::output_webhooks(&data, cfg.output)?;
        }
        WebhooksCmd::Get { id } => {
            let data = fetch_get(&client, &ws, &id).await?;
            render::webhook_detail(&data, cfg.output)?;
        }
        WebhooksCmd::Create {
            url,
            type_,
            event,
            label,
            ancestor,
        } => {
            safety::enforce_read_only(g.read_only, "webhooks create")?;
            let mut body = Map::new();
            body.insert("url".into(), url.into());
            body.insert("type".into(), type_.into());
            body.insert("event".into(), event.into());
            if let Some(l) = label {
                body.insert("label".into(), l.into());
            }
            if let Some(a) = ancestor {
                body.insert("ancestorId".into(), a.into());
            }
            let data = create_webhook(&client, &ws, &Value::Object(body)).await?;
            render::webhook_detail(&data, cfg.output)?;
        }
        WebhooksCmd::Delete { id } => {
            safety::enforce_read_only(g.read_only, "webhooks delete")?;
            safety::confirm_destructive(&format!("Delete webhook '{id}'?"), g.yes)?;
            delete_webhook(&client, &ws, &id).await?;
            println!("Webhook deleted ({id}).");
        }
    }
    Ok(())
}
