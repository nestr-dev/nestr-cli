use std::collections::{HashSet, VecDeque};
use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::{OutputFormat, ResolvedConfig};
use crate::error::NestrError;
use crate::render::{self, print_json};

#[derive(Args)]
pub struct ActivityArgs {
    /// Another user's activity, across the workspaces you share (default: your own).
    #[arg(long, value_name = "USER_ID")]
    pub user: Option<String>,
    /// Your own activity only: show agent considerations from your DM with this user.
    #[arg(long, value_name = "USER_ID", conflicts_with = "user")]
    pub with_user: Option<String>,
    /// Max entries (server default 50, max 200).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=200))]
    pub limit: Option<u32>,
}

/// Activity entries, newest first. `user` None is the caller's own feed.
pub async fn fetch(
    client: &NestrClient,
    user: Option<&str>,
    limit: Option<u32>,
    with_user: Option<&str>,
) -> crate::error::Result<Value> {
    let limit = limit.map(|n| n.to_string());
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(l) = &limit {
        params.push(("limit", l));
    }
    if let Some(w) = with_user {
        params.push(("withUser", w));
    }
    let path = match user {
        Some(id) => format!("/users/{id}/activity"),
        None => "/users/me/activity".to_string(),
    };
    let raw: Value = client.get(&path, &params).await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data
        .get("activity")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![])))
}

/// One line saying what an entry did: the description, or for an agent
/// consideration its summary with the tools used and the outcome.
pub fn what(entry: &Value) -> String {
    let s = |k: &str| entry.get(k).and_then(Value::as_str).unwrap_or("");
    let base = if s("description").is_empty() {
        s("summary")
    } else {
        s("description")
    };
    let mut extra: Vec<String> = Vec::new();
    let tools: Vec<&str> = entry
        .get("tools")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !tools.is_empty() {
        extra.push(format!("tools: {}", tools.join(", ")));
    }
    if !s("outcome").is_empty() {
        extra.push(format!("outcome: {}", s("outcome")));
    }
    let line = if extra.is_empty() {
        base.to_string()
    } else {
        format!("{base} ({})", extra.join("; "))
    };
    render::clean_text(&line)
}

fn when(entry: &Value) -> String {
    let d = entry.get("date").and_then(Value::as_str).unwrap_or("");
    render::clean_text(&d.chars().take(16).collect::<String>().replace('T', " "))
}

pub async fn run(args: ActivityArgs, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let data = fetch(
        &client,
        args.user.as_deref(),
        args.limit,
        args.with_user.as_deref(),
    )
    .await?;
    if cfg.output == OutputFormat::Json {
        return print_json(&data);
    }
    print_entries(data.as_array().map(Vec::as_slice).unwrap_or_default());
    Ok(())
}

/// Text mode: activity entries as a table, in the order given.
pub fn print_entries(entries: &[Value]) {
    if entries.is_empty() {
        render::print_no_results("No activity.");
        return;
    }
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|e| {
            let s = |k: &str| render::clean_text(e.get(k).and_then(Value::as_str).unwrap_or(""));
            vec![when(e), s("type"), s("nestTitle"), s("nestId"), what(e)]
        })
        .collect();
    println!(
        "{}",
        render::format_table(&["DATE", "TYPE", "NEST", "NEST ID", "WHAT"], rows)
    );
}

// ---- nest activity: GET /nests/:id/activity --------------------------------

/// Pages `--since` reads back through before it gives up on finding the entry.
pub const SINCE_PAGE_CAP: usize = 20;
/// Page size for `--since` and `--follow` when no `--limit` is given: the API's
/// maximum, so a burst costs the fewest requests.
pub const PAGING_PAGE_SIZE: u32 = 200;
/// How many printed entry ids `--follow` remembers to avoid printing one twice.
const SEEN_CAP: usize = 5000;

#[derive(Args)]
pub struct NestActivityArgs {
    /// The nest whose activity to read: it and, by default, everything beneath it.
    pub id: String,
    /// Entries per page, newest first (server default 50, max 200). With --since or
    /// --follow it is the page size used while paging back (default 200 there).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=200))]
    pub limit: Option<u32>,
    /// How far below the nest to look: a number of levels (0 is the nest alone) or
    /// `all` (the server default).
    #[arg(long, value_name = "N|all", value_parser = parse_depth)]
    pub depth: Option<String>,
    /// Only the entries newer than this entry id. Pages back through the API's
    /// `links.next` until it reaches that entry (at most 20 pages), so a burst larger
    /// than one page is not lost.
    #[arg(long, value_name = "ENTRY_ID")]
    pub since: Option<String>,
    /// Keep polling, and print each new entry to stdout as one JSON line, oldest
    /// first. Starts after --since when given, otherwise after the newest entry at
    /// start (which is not printed). A failed poll is reported on stderr and retried
    /// on the next interval; no entry is lost or printed twice. A rejected login, a
    /// refused right or a missing nest ends it with the error instead.
    #[arg(long)]
    pub follow: bool,
    /// Seconds between polls with --follow.
    #[arg(long, value_name = "SECONDS", default_value_t = 120, requires = "follow",
          value_parser = clap::value_parser!(u64).range(1..))]
    pub interval: u64,
}

/// `--depth`: a non-negative number of levels, or `all`.
pub fn parse_depth(s: &str) -> std::result::Result<String, String> {
    if s == "all" || s.parse::<u32>().is_ok() {
        Ok(s.to_string())
    } else {
        Err(format!(
            "expected a number of levels (0 or more) or `all`, got '{s}'"
        ))
    }
}

/// The `before` cursor carried on a `links.next` URL.
pub fn cursor_from_next(next: &str) -> Option<String> {
    let url = url::Url::parse(next).ok()?;
    url.query_pairs()
        .find(|(k, _)| k == "before")
        .map(|(_, v)| v.into_owned())
        .filter(|v| !v.is_empty())
}

/// The query params a nest activity read passes through.
#[derive(Debug, Clone, Default)]
pub struct NestActivityQuery {
    pub limit: Option<u32>,
    pub depth: Option<String>,
}

/// One page of a nest's activity, newest first, and the cursor for the next
/// (older) page when there is one.
#[derive(Debug, Clone)]
pub struct NestActivityPage {
    pub entries: Vec<Value>,
    pub next: Option<String>,
}

fn entry_id(entry: &Value) -> Option<&str> {
    entry.get("id").and_then(Value::as_str)
}

fn entry_date(entry: &Value) -> Option<&str> {
    entry.get("date").and_then(Value::as_str)
}

pub async fn fetch_nest_page(
    client: &NestrClient,
    id: &str,
    q: &NestActivityQuery,
    before: Option<&str>,
) -> crate::error::Result<NestActivityPage> {
    let limit = q.limit.map(|n| n.to_string());
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(l) = &limit {
        params.push(("limit", l));
    }
    if let Some(d) = &q.depth {
        params.push(("depth", d));
    }
    if let Some(b) = before {
        params.push(("before", b));
    }
    let raw: Value = client
        .get(&format!("/nests/{id}/activity"), &params)
        .await?;
    let (data, meta, links) = unwrap_data(raw);
    let entries = match data {
        Value::Array(a) => a,
        Value::Object(mut m) => match m.remove("activity") {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let has_more = meta
        .as_ref()
        .and_then(|m| m.get("has_more"))
        .and_then(Value::as_bool);
    let next = links
        .as_ref()
        .and_then(|l| l.get("next"))
        .and_then(Value::as_str)
        .and_then(cursor_from_next)
        .filter(|_| has_more != Some(false));
    Ok(NestActivityPage { entries, next })
}

/// The entries newer than a given entry, newest first. `reached` is false when the
/// entry was not found before the list ended or the page cap was hit; `entries` then
/// holds everything read. `anchor_date` is the `date` of the `since` entry when it
/// was reached.
#[derive(Debug, Clone)]
pub struct Since {
    pub entries: Vec<Value>,
    pub reached: bool,
    pub anchor_date: Option<String>,
}

/// Read pages, newest first, until the entry `since` turns up, following the
/// `links.next` cursor for at most `max_pages` pages.
pub async fn fetch_nest_since(
    client: &NestrClient,
    id: &str,
    q: &NestActivityQuery,
    since: &str,
    max_pages: usize,
) -> crate::error::Result<Since> {
    let mut entries = Vec::new();
    let mut before: Option<String> = None;
    for _ in 0..max_pages.max(1) {
        let page = fetch_nest_page(client, id, q, before.as_deref()).await?;
        for e in page.entries {
            if entry_id(&e) == Some(since) {
                return Ok(Since {
                    entries,
                    reached: true,
                    anchor_date: entry_date(&e).map(str::to_string),
                });
            }
            entries.push(e);
        }
        match page.next {
            Some(c) if before.as_deref() != Some(c.as_str()) => before = Some(c),
            _ => break,
        }
    }
    Ok(Since {
        entries,
        reached: false,
        anchor_date: None,
    })
}

/// Ids already printed by `--follow`, bounded so a long run stays small.
#[derive(Debug, Default)]
struct Seen {
    order: VecDeque<String>,
    set: HashSet<String>,
}

impl Seen {
    /// True when the id is new (and remembers it).
    fn insert(&mut self, id: &str) -> bool {
        if !self.set.insert(id.to_string()) {
            return false;
        }
        self.order.push_back(id.to_string());
        if self.order.len() > SEEN_CAP {
            if let Some(old) = self.order.pop_front() {
                self.set.remove(&old);
            }
        }
        true
    }
}

/// The state of `--follow`: the newest entry handled so far (the anchor the next
/// poll reads back to) and the ids already printed.
#[derive(Debug)]
pub struct Follower {
    id: String,
    q: NestActivityQuery,
    anchor: Option<String>,
    /// The anchor's `date`, once known. When the anchor goes missing, entries
    /// older than it are history from before this point, not news.
    anchor_date: Option<String>,
    seen: Seen,
    /// Set by a poll that did not find its anchor (deleted, or more than
    /// [`SINCE_PAGE_CAP`] pages back); the caller reports it.
    pub missed_anchor: Option<String>,
}

impl Follower {
    /// Start after `since`, or after the newest entry right now when it is None.
    pub async fn start(
        client: &NestrClient,
        id: &str,
        q: NestActivityQuery,
        since: Option<String>,
    ) -> crate::error::Result<Self> {
        let (anchor, anchor_date) = match since {
            Some(s) => (Some(s), None),
            None => {
                let newest = NestActivityQuery {
                    limit: Some(1),
                    depth: q.depth.clone(),
                };
                let page = fetch_nest_page(client, id, &newest, None).await?;
                let first = page.entries.first();
                (
                    first.and_then(entry_id).map(str::to_string),
                    first.and_then(entry_date).map(str::to_string),
                )
            }
        };
        let mut seen = Seen::default();
        if let Some(a) = &anchor {
            seen.insert(a);
        }
        Ok(Self {
            id: id.to_string(),
            q,
            anchor,
            anchor_date,
            seen,
            missed_anchor: None,
        })
    }

    /// The entries that arrived since the last successful poll, oldest first. On an
    /// error nothing moves, so the next poll reads the same span again.
    pub async fn poll(&mut self, client: &NestrClient) -> crate::error::Result<Vec<Value>> {
        let entries = match &self.anchor {
            Some(a) => {
                let mut s = fetch_nest_since(client, &self.id, &self.q, a, SINCE_PAGE_CAP).await?;
                if s.reached {
                    if self.anchor_date.is_none() {
                        self.anchor_date = s.anchor_date;
                    }
                } else {
                    self.missed_anchor = Some(a.clone());
                    // Wait after the newest entry read from now on, even if none of
                    // it is printed, so the next poll does not miss the same anchor.
                    if let Some(newest) = s.entries.iter().find(|e| entry_id(e).is_some()) {
                        self.anchor = entry_id(newest).map(str::to_string);
                    }
                    // ISO 8601 UTC strings from one API compare in time order. An
                    // entry at the anchor's own instant is kept: the seen set stops
                    // a repeat, and dropping it could lose a real new entry.
                    if let Some(d) = self.anchor_date.clone() {
                        s.entries
                            .retain(|e| entry_date(e).is_none_or(|x| x >= d.as_str()));
                    }
                }
                s.entries
            }
            // Nothing to read back to yet (the nest had no activity at start).
            None => {
                fetch_nest_page(client, &self.id, &self.q, None)
                    .await?
                    .entries
            }
        };
        if let Some(newest) = entries.iter().find(|e| entry_id(e).is_some()) {
            self.anchor = entry_id(newest).map(str::to_string);
            self.anchor_date = entry_date(newest).map(str::to_string);
        }
        Ok(entries
            .into_iter()
            .rev()
            .filter(|e| entry_id(e).is_none_or(|id| self.seen.insert(id)))
            .collect())
    }
}

/// `nestr nests activity <id>`.
pub async fn run_nest(
    args: NestActivityArgs,
    cfg: &ResolvedConfig,
    client: &NestrClient,
) -> Result<()> {
    let paging = args.since.is_some() || args.follow;
    let q = NestActivityQuery {
        limit: args.limit.or(paging.then_some(PAGING_PAGE_SIZE)),
        depth: args.depth,
    };
    if args.follow {
        return follow(
            client,
            &args.id,
            q,
            args.since,
            Duration::from_secs(args.interval),
        )
        .await;
    }
    let entries = match &args.since {
        Some(since) => {
            let s = fetch_nest_since(client, &args.id, &q, since, SINCE_PAGE_CAP).await?;
            if !s.reached {
                warn_missed(since);
            }
            s.entries
        }
        None => fetch_nest_page(client, &args.id, &q, None).await?.entries,
    };
    if cfg.output == OutputFormat::Json {
        return print_json(&Value::Array(entries));
    }
    print_entries(&entries);
    Ok(())
}

fn warn_missed(since: &str) {
    eprintln!(
        "warning: entry '{}' was not found within {SINCE_PAGE_CAP} pages of this nest's \
         activity; showing every entry read",
        render::sanitize(since)
    );
}

/// Whether `--follow` should retry after this error. A server error, a rate limit
/// or a network failure can clear by the next interval; a rejected credential, a
/// refused right, a missing nest or a bad request cannot, so those end the follow
/// with the error and a non-zero exit, where a supervising script can see it.
pub fn is_transient(e: &NestrError) -> bool {
    matches!(
        e,
        NestrError::Api { .. } | NestrError::Http(_) | NestrError::Json(_) | NestrError::Io(_)
    )
}

async fn follow(
    client: &NestrClient,
    id: &str,
    q: NestActivityQuery,
    since: Option<String>,
    interval: Duration,
) -> Result<()> {
    // From --since, the first poll runs at once; from "now" there is nothing to
    // read until the first interval has passed.
    let mut wait = since.is_none();
    let mut f = Follower::start(client, id, q, since).await?;
    loop {
        if wait {
            tokio::time::sleep(interval).await;
        }
        wait = true;
        match f.poll(client).await {
            Ok(entries) => {
                if let Some(a) = f.missed_anchor.take() {
                    warn_missed(&a);
                }
                let mut out = std::io::stdout().lock();
                for e in &entries {
                    writeln!(out, "{}", serde_json::to_string(e)?)?;
                }
                out.flush()?;
            }
            Err(e) if is_transient(&e) => eprintln!(
                "nestr: polling activity failed, retrying in {}s: {e}",
                interval.as_secs()
            ),
            Err(e) => return Err(e.into()),
        }
    }
}
