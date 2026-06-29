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
        /// The nest's name — what shows in lists.
        #[arg(long)]
        title: String,
        /// Parent nest id. Makes this a child/subtask of that nest (omit for a top-level nest).
        #[arg(long)]
        parent: Option<String>,
        /// One-line statement of *why* this exists. Optional for a project (inherited from
        /// the parent if unset); central for circles and roles. Keep it to a single line —
        /// the body goes in --description, never here.
        #[arg(long)]
        purpose: Option<String>,
        /// The body: the actual details/content of the nest. Renders as Markdown in the
        /// web app; literal angle brackets (`<id>`, `x < y`) are stripped by the server's
        /// HTML sanitizer — write `&lt;id&gt;` to keep them.
        #[arg(long)]
        description: Option<String>,
        /// Prime label sets what the nest *is*: project, goal, result, checklist, meeting,
        /// metric, feedback, circle, role, anchor-circle, tension. Omit for a plain todo.
        /// Repeatable, but at most one prime label (others may be free-form, e.g. urgent).
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Due date, ISO format (e.g. 2026-07-01).
        #[arg(long)]
        due: Option<String>,
        /// Assign user(s) to the nest by id (repeatable). Sets the nest's `users`.
        /// Pass the literal `me` to assign yourself. A project/task with no assignee
        /// shows up under nobody's work — assign whoever does the work.
        #[arg(long = "assignee")]
        assignees: Vec<String>,
    },
    /// Update fields on a nest.
    Update {
        id: String,
        /// The nest's name.
        #[arg(long)]
        title: Option<String>,
        /// One-line *why* (inherited from the parent if unset). Body text goes in --description.
        #[arg(long)]
        purpose: Option<String>,
        /// The body: the actual details/content of the nest. Renders as Markdown in the
        /// web app; literal angle brackets (`<id>`, `x < y`) are stripped by the server's
        /// HTML sanitizer — write `&lt;id&gt;` to keep them.
        #[arg(long)]
        description: Option<String>,
        /// Due date, ISO format (e.g. 2026-07-01).
        #[arg(long)]
        due: Option<String>,
        /// Mark the nest done (or not).
        #[arg(long)]
        completed: Option<bool>,
        /// Move the nest under a new parent id.
        #[arg(long)]
        parent: Option<String>,
        /// Replace the label set (repeatable) — sends the full set, not a delta, so re-list
        /// any labels you want to keep. At most one prime label. To toggle a single label
        /// without touching the rest, use `nests label add/remove`.
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Replace the assigned user(s) by id (repeatable) — sends the full set, so re-list
        /// anyone you want to keep. Pass the literal `me` for yourself.
        #[arg(long = "assignee")]
        assignees: Vec<String>,
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

/// Resolve `--assignee` tokens into the `users` array Nestr stores on a nest. Ids
/// pass through unchanged; the literal `me` is expanded to the authenticated user's
/// id via a single `/users/me` lookup (performed only when `me` is present, so the
/// common explicit-id case stays network-free).
pub async fn resolve_assignees(
    client: &NestrClient,
    assignees: &[String],
) -> crate::error::Result<Vec<String>> {
    if !assignees.iter().any(|a| a == "me") {
        return Ok(assignees.to_vec());
    }
    let me = crate::commands::me::fetch_me(client).await?;
    let my_id = me
        .get("_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if my_id.is_empty() {
        return Err(crate::error::NestrError::Validation(
            "could not resolve `--assignee me`: /users/me returned no id".into(),
        ));
    }
    Ok(assignees
        .iter()
        .map(|a| if a == "me" { my_id.clone() } else { a.clone() })
        .collect())
}

/// Assemble the `POST /nests` body. Pure so the field mapping — including the
/// `users` assignment that an unassigned project would otherwise miss — is unit
/// testable without a client. Optional fields are omitted (not sent as null/empty).
pub fn create_body(
    title: String,
    parent: Option<String>,
    purpose: Option<String>,
    description: Option<String>,
    labels: &[String],
    due: Option<String>,
    users: &[String],
) -> Value {
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
        body.insert("labels".into(), Value::from(labels.to_vec()));
    }
    if let Some(d) = due {
        body.insert("due".into(), d.into());
    }
    if !users.is_empty() {
        body.insert("users".into(), Value::from(users.to_vec()));
    }
    Value::Object(body)
}

/// Assemble the `PATCH /nests/{id}` body. Only fields the caller actually passed are
/// sent; `labels` and `users` are full-set replacements (re-list anything to keep).
#[allow(clippy::too_many_arguments)]
pub fn update_body(
    title: Option<String>,
    purpose: Option<String>,
    description: Option<String>,
    due: Option<String>,
    completed: Option<bool>,
    parent: Option<String>,
    labels: &[String],
    users: &[String],
) -> Value {
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
        body.insert("labels".into(), Value::from(labels.to_vec()));
    }
    if !users.is_empty() {
        body.insert("users".into(), Value::from(users.to_vec()));
    }
    Value::Object(body)
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

/// Guard `nests label add` so it can't give a nest a second prime label. When the label
/// being added is a prime, fetch the nest and reject early if it already carries a different
/// prime — mirroring the one-prime rule that `create`/`update --label` enforce. Adding a
/// non-prime label skips the fetch and is always allowed.
pub async fn ensure_prime_compatible(client: &NestrClient, id: &str, label_id: &str) -> Result<()> {
    if !validation::is_prime(label_id) {
        return Ok(());
    }
    let nest = fetch_get(client, id, &[]).await?;
    let existing = nest
        .get("labels")
        .and_then(Value::as_array)
        .map(|a| crate::views::label_codes(a))
        .unwrap_or_default();
    validation::validate_added_prime(&existing, label_id)
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
                render::output_nest_detail(&data, &cfg.host, cfg.output)?;
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
            assignees,
        } => {
            safety::enforce_read_only(g.read_only, "nests create")?;
            validation::validate_prime_labels(&labels)?;
            let users = resolve_assignees(&client, &assignees).await?;
            let body = create_body(title, parent, purpose, description, &labels, due, &users);
            let data = create_nest(&client, &body).await?;
            render::output_nest_detail(&data, &cfg.host, cfg.output)?;
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
            assignees,
        } => {
            safety::enforce_read_only(g.read_only, "nests update")?;
            if !labels.is_empty() {
                validation::validate_prime_labels(&labels)?;
            }
            let users = resolve_assignees(&client, &assignees).await?;
            let body = update_body(
                title,
                purpose,
                description,
                due,
                completed,
                parent,
                &labels,
                &users,
            );
            let data = update_nest(&client, &id, &body).await?;
            render::output_nest_detail(&data, &cfg.host, cfg.output)?;
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
            render::output_nest_detail(&data, &cfg.host, cfg.output)?;
        }
        NestsCmd::BulkReorder { ids } => {
            safety::enforce_read_only(g.read_only, "bulk-reorder")?;
            let data = bulk_reorder(&client, cfg.require_workspace()?, &ids).await?;
            render::output_nests(&data, None, cfg.output, false)?;
        }
        NestsCmd::Label { cmd } => {
            safety::enforce_read_only(g.read_only, "nests label")?;
            let data = match cmd {
                LabelCmd::Add { id, label_id } => {
                    ensure_prime_compatible(&client, &id, &label_id).await?;
                    set_label(&client, &id, &label_id, true).await?
                }
                LabelCmd::Remove { id, label_id } => {
                    set_label(&client, &id, &label_id, false).await?
                }
            };
            render::output_nest_detail(&data, &cfg.host, cfg.output)?;
        }
    }
    Ok(())
}
