use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::error::NestrError;
use crate::render;

#[derive(Subcommand)]
pub enum InsightsCmd {
    /// List workspace insights (metrics) for a period, each with its value and the
    /// value it compares against. One circle (--nest) or one person (--user) needs
    /// a Pro plan.
    List {
        /// One person's insights (Pro): `me` for yourself, which needs a token that
        /// acts for you. Cannot be combined with --nest.
        #[arg(long, conflicts_with = "nest")]
        user: Option<String>,
        /// One circle's insights (Pro). Defaults to the workspace.
        #[arg(long, alias = "circle")]
        nest: Option<String>,
        /// Include sub-circles' activity (server default: true).
        #[arg(long, num_args = 0..=1, default_missing_value = "true")]
        include_sub_circles: Option<bool>,
        /// Start of the period (ISO 8601). Defaults to one month before --to. The
        /// compare period has the same length and ends here.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now.
        #[arg(long, alias = "end-date")]
        to: Option<String>,
    },
    /// Get one metric by its type id, with its value for a period.
    Get {
        metric_id: String,
        /// Start of the period (ISO 8601). Defaults to one month before --to.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now.
        #[arg(long)]
        to: Option<String>,
    },
    /// Show a metric's history: the workspace's own, one circle's (--nest, Pro) or
    /// one person's (--user, Pro). The points of a metric nest, goal or other
    /// dataseries field are under `nests series` instead.
    History {
        metric_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        /// At most this many points (server default 1000).
        #[arg(long)]
        limit: Option<u32>,
        /// One circle's series (Pro). Defaults to the workspace.
        #[arg(long, alias = "circle")]
        nest: Option<String>,
        /// One person's series (Pro): `me` for yourself. Cannot be combined with --nest.
        #[arg(long, conflicts_with = "nest")]
        user: Option<String>,
    },
    /// Compare one metric across the direct child circles of a circle (Pro). The
    /// parent is listed too, but its value includes its children, so it is set apart.
    Compare {
        metric_id: String,
        /// The circle whose child circles to compare. Defaults to the workspace.
        #[arg(long)]
        parent: Option<String>,
        /// Start of the period (ISO 8601). Defaults to one month before --to.
        #[arg(long)]
        from: Option<String>,
        /// End of the period (ISO 8601). Defaults to now; runs to the end of that day.
        #[arg(long)]
        to: Option<String>,
        /// Whether each circle's value includes its sub-circles (server default: true).
        #[arg(long, num_args = 0..=1, default_missing_value = "true")]
        include_sub_circles: Option<bool>,
    },
}

/// Who and when an insights list covers.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub user: Option<String>,
    pub nest: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub include_sub_circles: Option<bool>,
}

/// The query `GET workspaces/:id/insights` takes for a scope.
pub fn list_params(s: &Scope) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    if let Some(u) = &s.user {
        params.push(("userId", u.clone()));
    }
    if let Some(n) = &s.nest {
        params.push(("nestId", n.clone()));
    }
    if let Some(b) = s.include_sub_circles {
        params.push(("includeSubCircles", b.to_string()));
    }
    if let Some(f) = &s.from {
        params.push(("from", f.clone()));
    }
    if let Some(t) = &s.to {
        params.push(("to", t.clone()));
        // Older servers (before `from`/`to`) read the end of the period from
        // `endDate`; newer ones take `to` and ignore `endDate` when both are sent.
        params.push(("endDate", t.clone()));
    }
    params
}

/// The insights route a reply came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    List,
    Get,
    History,
    Compare,
}

impl Route {
    fn command(self) -> &'static str {
        match self {
            Route::List => "insights list",
            Route::Get => "insights get",
            Route::History => "insights history",
            Route::Compare => "insights compare",
        }
    }
}

/// The options that narrow what an insights reply covers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Asked<'a> {
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub nest: Option<&'a str>,
    pub user: Option<&'a str>,
}

/// Days since 1970-01-01 of an ISO 8601 date or timestamp (its `YYYY-MM-DD` part).
pub fn parse_day(s: &str) -> Option<i64> {
    let d = s.get(..10)?;
    let b = d.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = d[..4].parse().ok()?;
    let m: i64 = d[5..7].parse().ok()?;
    let day: i64 = d[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Today, in days since 1970-01-01 (UTC).
pub fn today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    (secs / 86_400) as i64
}

/// An older server (production before the insights periods shipped) starts every
/// list period one month before today, whatever the end, so an end this many days
/// back or more would give a period that runs backwards.
const OLD_SERVER_MAX_TO_AGE_DAYS: i64 = 28;

fn ignored(opt: &str, route: Route, covers: &str) -> NestrError {
    NestrError::Ignored(format!(
        "this Nestr server ignores {opt} on `nestr {}`, so its numbers cover {covers}, not what \
         you asked for. They were not printed. {opt} needs a newer Nestr server.",
        route.command()
    ))
}

/// Check that an insights reply covers what was asked, before it is shown.
///
/// A newer server states the period it analysed in `meta.period` (list, get,
/// compare) and the circle or person in `meta.nestId`/`meta.userId` (history). An
/// older one sends no `meta` and silently ignores what it doesn't know: `from` and
/// `to` everywhere but history (list takes `endDate` instead), and history's
/// `nestId`/`userId`. Whatever it ignored is refused here rather than shown as if it
/// applied. `Ok(Some(note))` means the reply stands but its period reads differently
/// from what the help says. `today` is in days since 1970-01-01.
pub fn confirm(
    route: Route,
    asked: &Asked,
    meta: Option<&Value>,
    today: i64,
) -> Result<Option<String>, NestrError> {
    let period_confirmed = meta
        .and_then(|m| m.get("period"))
        .is_some_and(Value::is_object);
    let echoed =
        |key: &str, want: &str| meta.and_then(|m| m.get(key)).and_then(Value::as_str) == Some(want);
    match route {
        Route::History => {
            if let Some(n) = asked.nest {
                if !echoed("nestId", n) {
                    return Err(ignored("--nest", route, "the whole workspace"));
                }
            }
            if let Some(u) = asked.user {
                if !echoed("userId", u) {
                    return Err(ignored("--user", route, "everyone"));
                }
            }
            Ok(None)
        }
        _ if period_confirmed => Ok(None),
        Route::List => {
            if asked.from.is_some() {
                return Err(ignored("--from", route, "a different period"));
            }
            let Some(to) = asked.to else {
                return Ok(None);
            };
            match parse_day(to) {
                Some(day) if today - day < OLD_SERVER_MAX_TO_AGE_DAYS => Ok(Some(format!(
                    "this Nestr server starts the period one month before today, not one month \
                     before --to, so these numbers cover a month ago until {to}."
                ))),
                _ => Err(NestrError::Ignored(format!(
                    "this Nestr server always starts the period one month before today, so \
                     --to {to} would give a period that runs backwards. The numbers were not printed. \
                     On this server use a --to within the last {OLD_SERVER_MAX_TO_AGE_DAYS} days \
                     (as YYYY-MM-DD); an earlier one needs a newer Nestr server."
                ))),
            }
        }
        Route::Get | Route::Compare => {
            if asked.from.is_some() {
                return Err(ignored("--from", route, "the last month"));
            }
            if asked.to.is_some() {
                return Err(ignored("--to", route, "the last month"));
            }
            Ok(None)
        }
    }
}

/// `me` stands for the caller: resolve it to their own id, which every server
/// accepts (only newer ones know `me`). Other ids pass through.
async fn resolve_user(
    client: &NestrClient,
    user: Option<String>,
) -> crate::error::Result<Option<String>> {
    match user.as_deref() {
        Some("me") => Ok(Some(crate::commands::me::my_id(client, "--user me").await?)),
        _ => Ok(user),
    }
}

/// A note about the period goes to stderr, as JSON under `-o json`.
fn emit_note(note: &str, output: OutputFormat) {
    match output {
        OutputFormat::Json => eprintln!("{}", serde_json::json!({ "note": note })),
        OutputFormat::Text => eprintln!("note: {note}"),
    }
}

/// Refuse a reply that doesn't cover what was asked; print any note about it.
fn check(
    route: Route,
    asked: &Asked,
    meta: Option<&Value>,
    output: OutputFormat,
) -> crate::error::Result<()> {
    if let Some(note) = confirm(route, asked, meta, today())? {
        emit_note(&note, output);
    }
    Ok(())
}

fn as_pairs<'a>(params: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    params.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

fn push_opt(params: &mut Vec<(&'static str, String)>, key: &'static str, v: Option<String>) {
    if let Some(v) = v {
        params.push((key, v));
    }
}

/// `GET workspaces/:id/insights`, with its meta (the periods analysed).
pub async fn fetch_list(
    client: &NestrClient,
    ws: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

/// `GET workspaces/:id/insights/:metric`, with its meta.
pub async fn fetch_get(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&format!("/workspaces/{ws}/insights/{metric}"), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

/// `GET workspaces/:id/insights/:metric/history`, with its meta (the circle and
/// person it covers, on newer servers).
pub async fn fetch_history(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(
            &format!("/workspaces/{ws}/insights/{metric}/history"),
            params,
        )
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

/// `GET workspaces/:id/insights/:metric/circles`, with its meta.
pub async fn fetch_compare(
    client: &NestrClient,
    ws: &str,
    metric: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(
            &format!("/workspaces/{ws}/insights/{metric}/circles"),
            params,
        )
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

fn print_with_meta(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    text: impl Fn(&Value, Option<&Value>) -> String,
) -> Result<()> {
    match output {
        OutputFormat::Json => {
            render::print_json(data)?;
            render::emit_meta(meta, output);
        }
        OutputFormat::Text => println!("{}", text(data, meta)),
    }
    Ok(())
}

pub async fn run(cmd: InsightsCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let ws = cfg.require_workspace()?.to_string();
    let output = cfg.output;
    match cmd {
        InsightsCmd::List {
            user,
            nest,
            include_sub_circles,
            from,
            to,
        } => {
            let user = resolve_user(&client, user).await?;
            let asked = Asked {
                from: from.as_deref(),
                to: to.as_deref(),
                nest: nest.as_deref(),
                user: user.as_deref(),
            };
            let params = list_params(&Scope {
                user: user.clone(),
                nest: nest.clone(),
                from: from.clone(),
                to: to.clone(),
                include_sub_circles,
            });
            let (data, meta) = fetch_list(&client, &ws, &as_pairs(&params)).await?;
            check(Route::List, &asked, meta.as_ref(), output)?;
            print_with_meta(&data, meta.as_ref(), output, render::insights_text)?;
        }
        InsightsCmd::Get {
            metric_id,
            from,
            to,
        } => {
            let asked = Asked {
                from: from.as_deref(),
                to: to.as_deref(),
                ..Default::default()
            };
            let mut params = Vec::new();
            push_opt(&mut params, "from", from.clone());
            push_opt(&mut params, "to", to.clone());
            let (data, meta) = fetch_get(&client, &ws, &metric_id, &as_pairs(&params)).await?;
            check(Route::Get, &asked, meta.as_ref(), output)?;
            print_with_meta(&data, meta.as_ref(), output, render::insight_detail_text)?;
        }
        InsightsCmd::History {
            metric_id,
            from,
            to,
            limit,
            nest,
            user,
        } => {
            let user = resolve_user(&client, user).await?;
            let asked = Asked {
                nest: nest.as_deref(),
                user: user.as_deref(),
                ..Default::default()
            };
            let mut params = Vec::new();
            push_opt(&mut params, "from", from);
            push_opt(&mut params, "to", to);
            push_opt(&mut params, "limit", limit.map(|n| n.to_string()));
            push_opt(&mut params, "nestId", nest.clone());
            push_opt(&mut params, "userId", user.clone());
            let (data, meta) = fetch_history(&client, &ws, &metric_id, &as_pairs(&params)).await?;
            check(Route::History, &asked, meta.as_ref(), output)?;
            render::output_history(&data, output)?;
        }
        InsightsCmd::Compare {
            metric_id,
            parent,
            from,
            to,
            include_sub_circles,
        } => {
            let asked = Asked {
                from: from.as_deref(),
                to: to.as_deref(),
                ..Default::default()
            };
            let mut params = Vec::new();
            push_opt(&mut params, "parentNestId", parent);
            push_opt(&mut params, "from", from.clone());
            push_opt(&mut params, "to", to.clone());
            push_opt(
                &mut params,
                "includeSubCircles",
                include_sub_circles.map(|b| b.to_string()),
            );
            let (data, meta) = fetch_compare(&client, &ws, &metric_id, &as_pairs(&params))
                .await
                .map_err(|e| match e {
                    NestrError::Unsupported { .. } => anyhow::anyhow!(
                        "{e}\nhint: comparing circles side by side is rolling out with that \
                         newer server. Until then, read one circle at a time with \
                         `nestr insights list --nest <circleId>`."
                    ),
                    e => e.into(),
                })?;
            check(Route::Compare, &asked, meta.as_ref(), output)?;
            print_with_meta(&data, meta.as_ref(), output, render::compare_text)?;
        }
    }
    Ok(())
}
