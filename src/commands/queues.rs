use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs, ReadFilter};
use crate::config::OutputFormat;
use crate::render;
use crate::render::print_json;
use crate::views::{DmThreadView, QueueView};

// A queue is a label on DM threads, not a space of its own. It hands back thread ids,
// which `nestr dm show` / `nestr dm posts` take directly.
#[derive(Subcommand)]
pub enum QueuesCmd {
    /// List the support queues you can see: the ones you monitor, plus any you have
    /// raised a thread in.
    List,
    /// List threads in a queue, most recently posted first. Subscribers see every
    /// thread; anyone else sees only the ones they raised.
    Threads {
        /// Queue key, e.g. `support` (from `nestr queues list`).
        key: String,
        #[command(flatten)]
        filter: ReadFilter,
    },
}

pub async fn fetch_list(client: &NestrClient) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me/queues", &[]).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Unlike the DM listing this answers with an object, `{ key, subscribed, threads }`.
pub async fn fetch_threads(
    client: &NestrClient,
    key: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/users/me/queues/{key}/threads"), params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: QueuesCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        QueuesCmd::List => {
            let data = fetch_list(&client).await?;
            match cfg.output {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => {
                    let queues: Vec<QueueView> =
                        serde_json::from_value(data.clone()).unwrap_or_default();
                    if queues.is_empty() {
                        render::print_no_results("No queues.");
                    } else {
                        println!("{}", render::queue_table(&queues));
                    }
                }
            }
        }
        QueuesCmd::Threads { key, filter } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(u) = filter.param() {
                params.push(("unread", u));
            }
            let data = fetch_threads(&client, &key, &params).await?;
            match cfg.output {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => {
                    let threads: Vec<DmThreadView> = data
                        .get("threads")
                        .cloned()
                        .and_then(|t| serde_json::from_value(t).ok())
                        .unwrap_or_default();
                    if data.get("subscribed").and_then(Value::as_bool) == Some(false) {
                        println!(
                            "Not subscribed to '{}': showing only threads you raised.",
                            render::clean_text(&key)
                        );
                    }
                    if threads.is_empty() {
                        render::print_no_results("No threads.");
                    } else {
                        println!("{}", render::queue_thread_table(&threads));
                    }
                }
            }
        }
    }
    Ok(())
}
