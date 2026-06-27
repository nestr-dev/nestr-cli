use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::{render, safety, validation};

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
    /// Create a nest.
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Label code (repeatable).
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        due: Option<String>,
    },
    /// Update fields on a nest.
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        completed: Option<bool>,
        #[arg(long)]
        parent: Option<String>,
        /// Replace the label set (repeatable).
        #[arg(long = "label")]
        labels: Vec<String>,
    },
    /// Delete a nest (soft delete).
    Delete { id: String },
    /// Move a nest before/after another.
    Reorder {
        id: String,
        #[arg(value_parser = ["before", "after"])]
        position: String,
        related_id: String,
    },
    /// Reorder a set of nests in the workspace to the given order.
    BulkReorder {
        /// Nest ids in the desired order.
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
    /// Add or remove a label on a nest.
    Label {
        #[command(subcommand)]
        cmd: LabelCmd,
    },
}

#[derive(Subcommand)]
pub enum LabelCmd {
    /// Add a label to a nest.
    Add { id: String, label_id: String },
    /// Remove a label from a nest.
    Remove { id: String, label_id: String },
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

pub async fn create_nest(client: &NestrClient, body: &Value) -> crate::error::Result<Value> {
    let raw: Value = client.post("/nests", body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn update_nest(
    client: &NestrClient,
    id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client.patch(&format!("/nests/{id}"), body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn delete_nest(client: &NestrClient, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client.delete(&format!("/nests/{id}")).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn reorder_nest(
    client: &NestrClient,
    id: &str,
    position: &str,
    related_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(
            &format!("/nests/{id}/reorder/{position}/{related_id}"),
            &serde_json::json!({}),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn bulk_reorder(
    client: &NestrClient,
    workspace_id: &str,
    ids: &[String],
) -> crate::error::Result<Value> {
    let body = serde_json::to_value(ids)?;
    let raw: Value = client
        .patch(&format!("/workspaces/{workspace_id}/reorder"), &body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn set_label(
    client: &NestrClient,
    id: &str,
    label_id: &str,
    add: bool,
) -> crate::error::Result<Value> {
    let verb = if add { "add_label" } else { "remove_label" };
    let raw: Value = client
        .patch(
            &format!("/nests/{id}/{verb}/{label_id}"),
            &serde_json::json!({}),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
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
                render::output_nests(&data, None, cfg.output, false)?;
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
            render::output_nests(&data, meta.as_ref(), cfg.output, true)?;
        }
        NestsCmd::Create {
            title,
            parent,
            purpose,
            description,
            labels,
            due,
        } => {
            safety::enforce_read_only(g.read_only, "nests create")?;
            validation::validate_prime_labels(&labels)?;
            let mut body = serde_json::Map::new();
            body.insert("title".into(), title.into());
            if let Some(p) = parent {
                body.insert("parentId".into(), p.into());
            }
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if let Some(d) = description {
                body.insert("description".into(), d.into());
            }
            if !labels.is_empty() {
                body.insert("labels".into(), serde_json::to_value(&labels)?);
            }
            if let Some(d) = due {
                body.insert("due".into(), d.into());
            }
            let data = create_nest(&client, &Value::Object(body)).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        NestsCmd::Update {
            id,
            title,
            purpose,
            description,
            due,
            completed,
            parent,
            labels,
        } => {
            safety::enforce_read_only(g.read_only, "nests update")?;
            if !labels.is_empty() {
                validation::validate_prime_labels(&labels)?;
            }
            let mut body = serde_json::Map::new();
            if let Some(t) = title {
                body.insert("title".into(), t.into());
            }
            if let Some(p) = purpose {
                body.insert("purpose".into(), p.into());
            }
            if let Some(d) = description {
                body.insert("description".into(), d.into());
            }
            if let Some(d) = due {
                body.insert("due".into(), d.into());
            }
            if let Some(c) = completed {
                body.insert("completed".into(), c.into());
            }
            if let Some(p) = parent {
                body.insert("parentId".into(), p.into());
            }
            if !labels.is_empty() {
                body.insert("labels".into(), serde_json::to_value(&labels)?);
            }
            let data = update_nest(&client, &id, &Value::Object(body)).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        NestsCmd::Delete { id } => {
            safety::enforce_read_only(g.read_only, "nests delete")?;
            safety::confirm_destructive(&format!("Delete nest '{id}'?"), g.yes)?;
            let data = delete_nest(&client, &id).await?;
            let msg = data
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("deleted");
            println!("{} ({id})", render::clean_text(msg));
        }
        NestsCmd::Reorder {
            id,
            position,
            related_id,
        } => {
            safety::enforce_read_only(g.read_only, "nests reorder")?;
            let data = reorder_nest(&client, &id, &position, &related_id).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
        NestsCmd::BulkReorder { ids } => {
            safety::enforce_read_only(g.read_only, "bulk-reorder")?;
            let data = bulk_reorder(&client, cfg.require_workspace()?, &ids).await?;
            render::output_nests(&data, None, cfg.output, false)?;
        }
        NestsCmd::Label { cmd } => {
            safety::enforce_read_only(g.read_only, "nests label")?;
            let (id, label_id, add) = match cmd {
                LabelCmd::Add { id, label_id } => (id, label_id, true),
                LabelCmd::Remove { id, label_id } => (id, label_id, false),
            };
            let data = set_label(&client, &id, &label_id, add).await?;
            render::output_nest_detail(&data, cfg.output)?;
        }
    }
    Ok(())
}
