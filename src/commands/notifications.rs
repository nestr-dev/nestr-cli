use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render::print_json;
use crate::views::NotificationView;
use crate::{render, safety};

#[derive(Subcommand)]
pub enum NotificationsCmd {
    /// List notifications (unread by default).
    List {
        /// all | me | relevant
        #[arg(long, value_parser = ["all", "me", "relevant"])]
        r#type: Option<String>,
        /// Filter by group (mentions, replies, direct_message, reactions, updates, governance).
        #[arg(long)]
        group: Option<String>,
        /// Include already-read notifications.
        #[arg(long)]
        show_read: bool,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        skip: Option<u32>,
    },
    /// Mark all notifications as read.
    Read,
}

pub async fn fetch_list(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me/notifications", params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn mark_read(client: &NestrClient) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            "/users/me/notifications/mark-all-read",
            &serde_json::json!({}),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn run(cmd: NotificationsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        NotificationsCmd::List {
            r#type,
            group,
            show_read,
            limit,
            skip,
        } => {
            let skip_n = skip.unwrap_or(0);
            let limit_n = limit;
            let limit = limit.map(|n| n.to_string());
            let skip = skip.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(t) = &r#type {
                params.push(("type", t));
            }
            if let Some(gr) = &group {
                params.push(("group", gr));
            }
            if show_read {
                params.push(("showRead", "true"));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(s) = &skip {
                params.push(("skip", s));
            }
            let data = fetch_list(&client, &params).await?;
            match cfg.output {
                crate::config::OutputFormat::Json => print_json(&data)?,
                crate::config::OutputFormat::Text => {
                    let items: Vec<NotificationView> =
                        serde_json::from_value(data.clone()).unwrap_or_default();
                    if items.is_empty() {
                        render::print_no_results("No notifications.");
                    } else {
                        let rows: Vec<Vec<String>> = items
                            .iter()
                            .map(|n| {
                                vec![
                                    if n.is_read { "" } else { "●" }.to_string(),
                                    n.group.clone().unwrap_or_default(),
                                    n.title.clone().unwrap_or_default(),
                                    n.actor_name.clone().unwrap_or_default(),
                                    n.created_at.clone().unwrap_or_default(),
                                ]
                            })
                            .collect();
                        let n = rows.len();
                        println!(
                            "{}",
                            render::format_table(&["", "GROUP", "TITLE", "BY", "AT"], rows)
                        );
                        if let Some(f) = render::skip_limit_footer(skip_n, limit_n, n) {
                            println!("{f}");
                        }
                    }
                }
            }
        }
        NotificationsCmd::Read => {
            safety::enforce_read_only(g.read_only, "notifications read")?;
            let data = mark_read(&client).await?;
            let n = data
                .get("markedCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            println!("Marked {n} notification(s) read.");
        }
    }
    Ok(())
}
