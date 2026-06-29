use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::render::print_json;
use crate::views::PostView;
use crate::{render, safety};

#[derive(Subcommand)]
pub enum CommentsCmd {
    /// List comments on a nest.
    List {
        nest_id: String,
        /// Include comments on descendant nests up to this depth (or "all").
        #[arg(long)]
        depth: Option<String>,
    },
    /// Add a comment to a nest.
    Add {
        nest_id: String,
        /// Comment text. Renders as Markdown in the web app (headings, **bold**, lists,
        /// `code`, links). Literal angle brackets are stripped by the server's HTML
        /// sanitizer — write `&lt;id&gt;` for a literal `<id>`.
        body: String,
        #[arg(long = "label")]
        labels: Vec<String>,
    },
    /// Edit a comment's text.
    Edit {
        comment_id: String,
        /// New comment text. Renders as Markdown; literal `<…>` is stripped by the
        /// server's HTML sanitizer (write `&lt;id&gt;` for a literal `<id>`).
        body: String,
    },
    /// Delete a comment.
    Delete { comment_id: String },
}

pub async fn fetch_list(
    client: &NestrClient,
    nest_id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/nests/{nest_id}/posts"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

// A comment is a nest with `type:"comment"` whose text lives in the nest's `title`
// field. The two endpoints below take DIFFERENT field names on purpose — match each
// route's documented OpenAPI schema, not each other:
//   - create → POST /nests/{id}/posts uses the `PostWrite` schema, which names the
//     text `body` (the server maps `body`→`title` internally; the response re-exposes
//     `title` as `body`).
//   - edit → PATCH /nests/{id} is the generic nest update (`NestWrite` schema), which
//     names the text `title`; there is no dedicated post-edit route.
// They look inconsistent but each is correct. Don't "align" edit to send `body`: that
// works only via undocumented mapping and could break — keep them matched to the schemas.
pub async fn add_comment(
    client: &NestrClient,
    nest_id: &str,
    body: &str,
    labels: &[String],
) -> crate::error::Result<Value> {
    let mut map = serde_json::Map::new();
    // `PostWrite` field name (see the note above on body-vs-title).
    map.insert("body".into(), body.into());
    map.insert("parentId".into(), nest_id.into());
    if !labels.is_empty() {
        map.insert("labels".into(), serde_json::to_value(labels)?);
    }
    let raw: Value = client
        .post(&format!("/nests/{nest_id}/posts"), &Value::Object(map))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn edit_comment(
    client: &NestrClient,
    comment_id: &str,
    body: &str,
) -> crate::error::Result<Value> {
    // `NestWrite` field name — the generic nest PATCH names the text `title`, not
    // `body` (see the note on add_comment above).
    let raw: Value = client
        .patch(&format!("/nests/{comment_id}"), &json!({ "title": body }))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn delete_comment(client: &NestrClient, comment_id: &str) -> crate::error::Result<Value> {
    let raw: Value = client.delete(&format!("/nests/{comment_id}")).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

fn render_posts(data: &Value, output: crate::config::OutputFormat) -> Result<()> {
    match output {
        crate::config::OutputFormat::Json => print_json(data)?,
        crate::config::OutputFormat::Text => {
            let posts: Vec<PostView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if posts.is_empty() {
                render::print_no_results("No comments.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = posts
                .iter()
                .map(|p| {
                    vec![
                        p.id.clone(),
                        p.created_by.clone().unwrap_or_default(),
                        p.text(),
                        p.created_at.clone().unwrap_or_default(),
                    ]
                })
                .collect();
            println!(
                "{}",
                render::format_table(&["ID", "BY", "BODY", "AT"], rows)
            );
        }
    }
    Ok(())
}

/// Render a single comment by its `body`. Comments store their text in `body`
/// (title is null), so the generic nest-detail renderer would print a blank line.
fn render_post_one(data: &Value, output: crate::config::OutputFormat) -> Result<()> {
    match output {
        crate::config::OutputFormat::Json => print_json(data)?,
        crate::config::OutputFormat::Text => {
            let p: PostView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("{}  [{}]", render::clean_text(&p.text()), p.id);
        }
    }
    Ok(())
}

pub async fn run(cmd: CommentsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    match cmd {
        CommentsCmd::List { nest_id, depth } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(d) = &depth {
                params.push(("depth", d));
            }
            let (data, _) = fetch_list(&client, &nest_id, &params).await?;
            render_posts(&data, cfg.output)?;
        }
        CommentsCmd::Add {
            nest_id,
            body,
            labels,
        } => {
            safety::enforce_read_only(g.read_only, "comments add")?;
            let data = add_comment(&client, &nest_id, &body, &labels).await?;
            render_post_one(&data, cfg.output)?;
        }
        CommentsCmd::Edit { comment_id, body } => {
            safety::enforce_read_only(g.read_only, "comments edit")?;
            let data = edit_comment(&client, &comment_id, &body).await?;
            render_post_one(&data, cfg.output)?;
        }
        CommentsCmd::Delete { comment_id } => {
            safety::enforce_read_only(g.read_only, "comments delete")?;
            safety::confirm_destructive(&format!("Delete comment '{comment_id}'?"), g.yes)?;
            let data = delete_comment(&client, &comment_id).await?;
            let msg = data
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("deleted");
            println!("{} ({comment_id})", render::clean_text(msg));
        }
    }
    Ok(())
}
