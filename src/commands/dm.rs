use anyhow::{bail, Result};
use clap::{ArgGroup, Subcommand};
use serde_json::{json, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs, ReadFilter};
use crate::config::OutputFormat;
use crate::render::print_json;
use crate::views::{DmThreadView, PostView};
use crate::{render, safety};

// A thread is the unit and its id is the whole address: the DM space that holds a pair's
// threads never appears in a URL or a response. These routes need a user-scoped (OAuth)
// login; a workspace API key is refused.
#[derive(Subcommand)]
pub enum DmCmd {
    /// List your open direct-message threads, most recently posted first.
    List {
        /// Only threads with this person: user id, username or email.
        /// `nestr_support` is your Nestradamus conversation.
        #[arg(long = "with", value_name = "USER")]
        with_user: Option<String>,
        /// Only threads with messages you have not read.
        #[arg(long)]
        unread: bool,
        /// Also list closed conversations (left out by default).
        #[arg(long)]
        include_closed: bool,
        /// Threads per page (default 50, max 200).
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        page: Option<u32>,
    },
    /// Start a new thread with someone you share a workspace with.
    Start {
        /// Who to message: user id, username or email.
        user: String,
        /// Thread title. Defaults to a dated one, as the app uses.
        #[arg(long)]
        title: Option<String>,
    },
    /// Show a thread: participants, status and unread count.
    /// `--unread` also embeds the posts you have not read.
    Show {
        thread_id: String,
        #[command(flatten)]
        filter: ReadFilter,
    },
    /// Read the posts in a thread, oldest first.
    Posts {
        thread_id: String,
        #[command(flatten)]
        filter: ReadFilter,
        /// Include posts on descendant nests up to this depth (or "all").
        #[arg(long)]
        depth: Option<String>,
    },
    /// Post a message into a thread.
    Post {
        thread_id: String,
        /// Message text. Supports Markdown.
        body: String,
    },
    /// Rename a thread or set who is in it.
    #[command(group(ArgGroup::new("change").required(true).multiple(true).args(["title", "users"])))]
    Update {
        thread_id: String,
        #[arg(long)]
        title: Option<String>,
        /// The full participant list you want (comma-separated user ids), replacing the
        /// current one. Read it with `dm show` first. Leave by sending it without yourself.
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        users: Option<Vec<String>>,
    },
    /// Close a conversation. It leaves `dm list` but stays readable and postable by id.
    Close { thread_id: String },
    /// Reopen a closed conversation.
    Reopen { thread_id: String },
    /// Mark read up to and including this post. Works for any post, not only DMs.
    MarkRead { post_id: String },
}

fn thread_path(thread_id: &str) -> String {
    format!("/users/me/dm/{thread_id}")
}

pub async fn fetch_list(
    client: &NestrClient,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get("/users/me/dm", params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn start_thread(
    client: &NestrClient,
    user: &str,
    title: Option<&str>,
) -> crate::error::Result<Value> {
    let mut body = json!({ "user": user });
    if let Some(t) = title {
        body["title"] = t.into();
    }
    let raw: Value = client.post("/users/me/dm", &body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_thread(
    client: &NestrClient,
    thread_id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client.get(&thread_path(thread_id), params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn fetch_posts(
    client: &NestrClient,
    thread_id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("{}/posts", thread_path(thread_id)), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

pub async fn post_message(
    client: &NestrClient,
    thread_id: &str,
    body: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(
            &format!("{}/posts", thread_path(thread_id)),
            &json!({ "body": body }),
        )
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The PATCH body is a partial nest: only the keys sent change.
pub fn update_body(title: Option<&str>, users: Option<&[String]>) -> Result<Value> {
    let mut body = serde_json::Map::new();
    if let Some(t) = title {
        body.insert("title".into(), t.into());
    }
    if let Some(u) = users {
        body.insert("users".into(), json!(u));
    }
    if body.is_empty() {
        bail!("Nothing to update: pass --title or --users.");
    }
    Ok(Value::Object(body))
}

pub fn close_body() -> Value {
    json!({ "completed": true })
}

/// `null`, not `false`: the API reads an absent `completed` as "leave it alone", so a
/// reopen has to send the key with a null value.
pub fn reopen_body() -> Value {
    json!({ "completed": null })
}

pub async fn update_thread(
    client: &NestrClient,
    thread_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client.patch(&thread_path(thread_id), body).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The read marker never moves backwards, so marking an older post is harmless.
pub async fn mark_read(client: &NestrClient, post_id: &str) -> crate::error::Result<Value> {
    let raw: Value = client
        .post(&format!("/posts/{post_id}/read"), &json!({}))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// `meta` is the posts route's paging, so a thread longer than one page says so
/// rather than looking complete.
fn print_posts(data: &Value, meta: Option<&Value>, empty: &str) {
    let posts: Vec<PostView> = serde_json::from_value(data.clone()).unwrap_or_default();
    if posts.is_empty() {
        render::print_no_results(empty);
    } else {
        println!("{}", render::post_table(&posts));
    }
    if let Some(f) = render::pagination_footer(meta, false) {
        println!("{f}");
    }
}

fn print_thread(data: &Value) {
    let t: DmThreadView = serde_json::from_value(data.clone()).unwrap_or_default();
    println!(
        "{}  [{}]",
        render::clean_text(t.title.as_deref().unwrap_or("")),
        t.id
    );
    println!(
        "  status: {}",
        if t.is_closed() { "closed" } else { "open" }
    );
    if !t.users.is_empty() {
        println!("  users:  {}", render::clean_text(&t.users.join(", ")));
    }
    if !t.ques.is_empty() {
        println!("  queues: {}", render::clean_text(&t.ques.join(", ")));
    }
    if let Some(hints) = data.get("hints").and_then(Value::as_array) {
        for h in hints {
            if let Some(label) = h.get("label").and_then(Value::as_str) {
                println!("  {}", render::clean_text(label));
            }
        }
    }
}

pub async fn run(cmd: DmCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let out = cfg.output;
    match cmd {
        DmCmd::List {
            with_user,
            unread,
            include_closed,
            limit,
            page,
        } => {
            let limit = limit.map(|n| n.to_string());
            let page = page.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(u) = &with_user {
                params.push(("user", u));
            }
            // The list route honours only unread=true, so there is no --read here.
            if unread {
                params.push(("unread", "true"));
            }
            if include_closed {
                params.push(("includeCompleted", "true"));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(p) = &page {
                params.push(("page", p));
            }
            let data = fetch_list(&client, &params).await?;
            match out {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => {
                    let threads: Vec<DmThreadView> =
                        serde_json::from_value(data.clone()).unwrap_or_default();
                    if threads.is_empty() {
                        render::print_no_results("No conversations.");
                    } else {
                        println!("{}", render::dm_thread_table(&threads, include_closed));
                    }
                }
            }
        }
        DmCmd::Start { user, title } => {
            safety::enforce_read_only(g.read_only, "dm start")?;
            let data = start_thread(&client, &user, title.as_deref()).await?;
            render::render_object(&data, out, print_thread)?;
        }
        DmCmd::Show { thread_id, filter } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(u) = filter.param() {
                params.push(("unread", u));
            }
            let data = fetch_thread(&client, &thread_id, &params).await?;
            match out {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => {
                    print_thread(&data);
                    if let Some(posts) = data.get("posts") {
                        println!();
                        print_posts(posts, data.get("postsMeta"), "No posts match.");
                    }
                }
            }
        }
        DmCmd::Posts {
            thread_id,
            filter,
            depth,
        } => {
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(u) = filter.param() {
                params.push(("unread", u));
            }
            if let Some(d) = &depth {
                params.push(("depth", d));
            }
            let (data, meta) = fetch_posts(&client, &thread_id, &params).await?;
            match out {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => print_posts(&data, meta.as_ref(), "No posts."),
            }
        }
        DmCmd::Post { thread_id, body } => {
            safety::enforce_read_only(g.read_only, "dm post")?;
            let data = post_message(&client, &thread_id, &body).await?;
            render::render_object(&data, out, |d| {
                let p: PostView = serde_json::from_value(d.clone()).unwrap_or_default();
                println!("Posted [{}]", p.id);
            })?;
        }
        DmCmd::Update {
            thread_id,
            title,
            users,
        } => {
            safety::enforce_read_only(g.read_only, "dm update")?;
            let body = update_body(title.as_deref(), users.as_deref())?;
            // A replace can drop people from the conversation, so it asks first.
            if let Some(u) = &users {
                safety::confirm_destructive(
                    &format!(
                        "Set the participants of thread '{thread_id}' to [{}]?",
                        u.join(", ")
                    ),
                    g.yes,
                )?;
            }
            let data = update_thread(&client, &thread_id, &body).await?;
            render::render_object(&data, out, print_thread)?;
        }
        DmCmd::Close { thread_id } => {
            safety::enforce_read_only(g.read_only, "dm close")?;
            let data = update_thread(&client, &thread_id, &close_body()).await?;
            render::render_object(&data, out, print_thread)?;
        }
        DmCmd::Reopen { thread_id } => {
            safety::enforce_read_only(g.read_only, "dm reopen")?;
            let data = update_thread(&client, &thread_id, &reopen_body()).await?;
            render::render_object(&data, out, print_thread)?;
        }
        DmCmd::MarkRead { post_id } => {
            safety::enforce_read_only(g.read_only, "dm mark-read")?;
            let data = mark_read(&client, &post_id).await?;
            render::render_object(&data, out, |d| {
                let at = d.get("readAt").and_then(Value::as_str).unwrap_or("");
                println!("Marked read up to {post_id} ({})", render::clean_text(at));
            })?;
        }
    }
    Ok(())
}
