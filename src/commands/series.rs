//! `nests series`: a nest's dataseries, the dated points behind a metric's history,
//! a goal's progress or a burndown.
//!
//! A series is addressed by the `key` `nests series list` prints (a field code, or
//! the field id when several fields share a code) plus, on a multi-series field,
//! its series id. A point is addressed by its own id, since a series can hold
//! several points on one day. The field a series tracks follows its newest point
//! at or before now, so a backfill or a correction re-syncs that field.

use anyhow::{bail, Result};
use clap::{Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::GlobalArgs;
use crate::config::OutputFormat;
use crate::error::NestrError;
use crate::{render, safety};

/// The most points the API takes in one write; a larger backfill is split.
pub const MAX_POINTS_PER_WRITE: usize = 500;

#[derive(Clone, Copy, ValueEnum)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    fn as_str(self) -> &'static str {
        match self {
            Order::Asc => "asc",
            Order::Desc => "desc",
        }
    }
}

#[derive(Subcommand)]
pub enum SeriesCmd {
    /// List the dataseries on a nest: each one's key, kind (point_in_time or
    /// cumulative), the field it tracks and its current point. A hidden series is
    /// not in use on this nest: it can be read, not written.
    List {
        /// The nest (a metric, goal, sprint, or any nest with a dataseries field).
        nest: String,
    },
    /// Read one series' dated points, oldest first.
    Points {
        nest: String,
        /// The series key from `nests series list`.
        key: String,
        /// The series id, on a multi-series field.
        #[arg(long = "series")]
        series_id: Option<String>,
        /// Earliest date, inclusive (ISO 8601 or epoch milliseconds).
        #[arg(long)]
        from: Option<String>,
        /// Latest date, inclusive (ISO 8601 or epoch milliseconds).
        #[arg(long)]
        to: Option<String>,
        /// At most this many points (server default 500, max 5000).
        #[arg(long)]
        limit: Option<u32>,
        /// Date order; `desc` for newest first.
        #[arg(long, value_enum)]
        order: Option<Order>,
    },
    /// Add dated points, e.g. to backfill past values: DATE=VALUE each, or a JSON
    /// file of `[{"date": …, "value": …}]`. Each point is kept exactly (several may
    /// share a day). The tracked field then follows the newest point at or before
    /// now; a future-dated point does not become the current value.
    Add {
        nest: String,
        /// The series key from `nests series list`.
        key: String,
        /// Points as DATE=VALUE, e.g. 2026-01-05=12 (DATE is ISO 8601 or epoch ms).
        points: Vec<String>,
        /// The series id, on a multi-series field.
        #[arg(long = "series")]
        series_id: Option<String>,
        /// Read points from a JSON file (`-` for stdin): an array of {date, value}.
        #[arg(long)]
        file: Option<String>,
    },
    /// Correct a point's date, value, or both. A corrected point is kept: a later
    /// value write to the tracked field no longer replaces it.
    Edit {
        nest: String,
        /// The series key from `nests series list`.
        key: String,
        /// The point id from `nests series points`.
        point_id: String,
        /// The series id, on a multi-series field.
        #[arg(long = "series")]
        series_id: Option<String>,
        /// New date (ISO 8601 or epoch milliseconds).
        #[arg(long)]
        date: Option<String>,
        /// New value (any finite number).
        #[arg(long, allow_negative_numbers = true)]
        value: Option<f64>,
    },
    /// Remove a point. The tracked field re-syncs to the newest point left at or
    /// before now; removing every point leaves the field as it is.
    Delete {
        nest: String,
        /// The series key from `nests series list`.
        key: String,
        /// The point id from `nests series points`.
        point_id: String,
        /// The series id, on a multi-series field.
        #[arg(long = "series")]
        series_id: Option<String>,
    },
}

fn points_path(nest: &str, key: &str, series_id: Option<&str>) -> String {
    match series_id {
        Some(s) => format!("/nests/{nest}/series/{key}/{s}/points"),
        None => format!("/nests/{nest}/series/{key}/points"),
    }
}

/// `GET nests/:id/series`.
pub async fn fetch_series(client: &NestrClient, nest: &str) -> crate::error::Result<Value> {
    let raw: Value = client.get(&format!("/nests/{nest}/series"), &[]).await?;
    Ok(unwrap_data(raw).0)
}

/// `GET nests/:id/series/:key[/:seriesId]/points`, with its meta (`has_more`).
pub async fn fetch_points(
    client: &NestrClient,
    nest: &str,
    key: &str,
    series_id: Option<&str>,
    params: &[(&str, &str)],
) -> crate::error::Result<(Value, Option<Value>)> {
    let raw: Value = client
        .get(&points_path(nest, key, series_id), params)
        .await?;
    let (data, meta, _) = unwrap_data(raw);
    Ok((data, meta))
}

/// What a backfill did: one result per point, in the order sent, each
/// `{index, status: created|error, data|message}`.
#[derive(Debug, Clone, Default)]
pub struct AddOutcome {
    pub created: usize,
    pub failed: usize,
    pub results: Vec<Value>,
    /// Set when a later request of a split backfill failed: the error, and how
    /// many points went out before it. Those are written, so a rerun of the whole
    /// backfill would add them twice.
    pub stopped: Option<(usize, String)>,
}

impl AddOutcome {
    /// Why the command exits non-zero, if it should.
    pub fn failure(&self, total: usize) -> Option<String> {
        if let Some((sent, err)) = &self.stopped {
            return Some(format!(
                "{err}\nPoints 1-{sent} were already sent ({} added, {} refused). \
                 Resend only from point #{} on, or the added points are stored twice.",
                self.created,
                self.failed,
                sent + 1
            ));
        }
        if self.failed == 0 {
            return None;
        }
        Some(if self.created == 0 {
            format!("all {} points were refused; nothing was added", self.failed)
        } else {
            format!(
                "{} of {total} points were refused; {} {} added",
                self.failed,
                self.created,
                if self.created == 1 { "was" } else { "were" }
            )
        })
    }
}

/// `POST …/points`, always as an array so every point gets its own result. Sends
/// at most [`MAX_POINTS_PER_WRITE`] per request; `index` runs across all of them.
/// A failed first request is an error (nothing was written); a failed later one
/// stops the backfill and is recorded in [`AddOutcome::stopped`].
pub async fn add_points(
    client: &NestrClient,
    nest: &str,
    key: &str,
    series_id: Option<&str>,
    points: &[Value],
) -> crate::error::Result<AddOutcome> {
    let path = points_path(nest, key, series_id);
    let mut outcome = AddOutcome::default();
    for (n, chunk) in points.chunks(MAX_POINTS_PER_WRITE).enumerate() {
        let offset = n * MAX_POINTS_PER_WRITE;
        let raw: Value = match client.post(&path, &Value::Array(chunk.to_vec())).await {
            Ok(raw) => raw,
            Err(e) if offset > 0 => {
                outcome.stopped = Some((offset, e.to_string()));
                return Ok(outcome);
            }
            Err(e) => return Err(e),
        };
        let (data, _, _) = unwrap_data(raw);
        for (i, mut r) in data
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            let index = r.get("index").and_then(Value::as_u64).unwrap_or(i as u64) as usize;
            if r.get("status").and_then(Value::as_str) == Some("created") {
                outcome.created += 1;
            } else {
                outcome.failed += 1;
            }
            r["index"] = json!(offset + index);
            outcome.results.push(r);
        }
    }
    Ok(outcome)
}

/// `PATCH …/points/:pointId`.
pub async fn update_point(
    client: &NestrClient,
    nest: &str,
    key: &str,
    series_id: Option<&str>,
    point_id: &str,
    body: &Value,
) -> crate::error::Result<Value> {
    let path = format!("{}/{point_id}", points_path(nest, key, series_id));
    let raw: Value = client.patch(&path, body).await?;
    Ok(unwrap_data(raw).0)
}

/// `DELETE …/points/:pointId`.
pub async fn delete_point(
    client: &NestrClient,
    nest: &str,
    key: &str,
    series_id: Option<&str>,
    point_id: &str,
) -> crate::error::Result<Value> {
    let path = format!("{}/{point_id}", points_path(nest, key, series_id));
    let raw: Value = client.delete(&path).await?;
    Ok(unwrap_data(raw).0)
}

fn finite(value: f64, what: &str) -> crate::error::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(NestrError::Validation(format!(
            "{what} must be a finite number"
        )))
    }
}

/// One `DATE=VALUE` argument as the point the API takes. The value is sent as a
/// JSON number (the API refuses a string); the date goes as written, since the
/// API reads ISO 8601 and epoch milliseconds itself.
pub fn parse_point(raw: &str) -> crate::error::Result<Value> {
    let bad = || {
        NestrError::Validation(format!(
            "point '{raw}' is not DATE=VALUE, e.g. 2026-01-05=12"
        ))
    };
    let (date, value) = raw.rsplit_once('=').ok_or_else(bad)?;
    let date = date.trim();
    if date.is_empty() {
        return Err(bad());
    }
    let value: f64 = value.trim().parse().map_err(|_| bad())?;
    let value = finite(value, &format!("the value in '{raw}'"))?;
    Ok(json!({ "date": date, "value": value }))
}

/// The body of an edit: only the keys that change, at least one.
pub fn edit_body(date: Option<String>, value: Option<f64>) -> crate::error::Result<Value> {
    let mut body = serde_json::Map::new();
    if let Some(d) = date {
        body.insert("date".into(), json!(d));
    }
    if let Some(v) = value {
        body.insert("value".into(), json!(finite(v, "--value")?));
    }
    if body.is_empty() {
        return Err(NestrError::Validation(
            "pass --date, --value, or both".into(),
        ));
    }
    Ok(Value::Object(body))
}

fn read_points_file(file: &str) -> Result<Vec<Value>> {
    let text = if file == "-" {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(file)?
    };
    let parsed: Value =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{file} is not JSON: {e}"))?;
    let items = match parsed {
        Value::Array(a) => a,
        Value::Object(mut m) => match m.remove("points") {
            Some(Value::Array(a)) => a,
            _ => bail!("{file} must hold an array of {{\"date\", \"value\"}} points"),
        },
        _ => bail!("{file} must hold an array of {{\"date\", \"value\"}} points"),
    };
    Ok(items)
}

/// What `nests series add` prints in text mode: the count, then each refused
/// point with the server's reason.
pub fn add_text(outcome: &AddOutcome, points: &[Value], key: &str) -> String {
    let plural = |n: usize| if n == 1 { "point" } else { "points" };
    let mut out = vec![format!(
        "Added {} {} to series '{}'.",
        outcome.created,
        plural(outcome.created),
        render::sanitize(key)
    )];
    for r in &outcome.results {
        if r.get("status").and_then(Value::as_str) == Some("created") {
            continue;
        }
        let index = r.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let sent = points
            .get(index)
            .map(|p| {
                let date = p.get("date").map(|d| match d {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
                let value = p.get("value").map(|v| match v.as_f64() {
                    Some(n) => render::fmt_num(Some(n)),
                    None => v.to_string(),
                });
                format!("{}={}", date.unwrap_or_default(), value.unwrap_or_default())
            })
            .unwrap_or_default();
        let msg = r
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("refused");
        out.push(format!(
            "  refused #{} {}: {}",
            index + 1,
            render::sanitize(&sent),
            render::sanitize(msg)
        ));
    }
    out.join("\n")
}

pub async fn run(cmd: SeriesCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = crate::commands::resolve_client(g).await?;
    let output = cfg.output;
    match cmd {
        SeriesCmd::List { nest } => {
            let data = fetch_series(&client, &nest).await?;
            match output {
                OutputFormat::Json => render::print_json(&data)?,
                OutputFormat::Text => println!("{}", render::series_text(&data)),
            }
        }
        SeriesCmd::Points {
            nest,
            key,
            series_id,
            from,
            to,
            limit,
            order,
        } => {
            let limit = limit.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if let Some(f) = &from {
                params.push(("from", f));
            }
            if let Some(t) = &to {
                params.push(("to", t));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            if let Some(o) = order {
                params.push(("order", o.as_str()));
            }
            let (data, meta) =
                fetch_points(&client, &nest, &key, series_id.as_deref(), &params).await?;
            match output {
                OutputFormat::Json => {
                    render::print_json(&data)?;
                    render::emit_meta(meta.as_ref(), output);
                }
                OutputFormat::Text => println!("{}", render::points_text(&data, meta.as_ref())),
            }
        }
        SeriesCmd::Add {
            nest,
            key,
            points,
            series_id,
            file,
        } => {
            safety::enforce_read_only(g.read_only, "nests series add")?;
            let mut all: Vec<Value> = Vec::new();
            if let Some(f) = &file {
                all.extend(read_points_file(f)?);
            }
            for p in &points {
                all.push(parse_point(p)?);
            }
            if all.is_empty() {
                bail!("no points to add: pass DATE=VALUE arguments or --file");
            }
            let outcome = add_points(&client, &nest, &key, series_id.as_deref(), &all).await?;
            match output {
                OutputFormat::Json => render::print_json(&Value::Array(outcome.results.clone()))?,
                OutputFormat::Text => println!("{}", add_text(&outcome, &all, &key)),
            }
            if let Some(why) = outcome.failure(all.len()) {
                bail!(why);
            }
        }
        SeriesCmd::Edit {
            nest,
            key,
            point_id,
            series_id,
            date,
            value,
        } => {
            safety::enforce_read_only(g.read_only, "nests series edit")?;
            let body = edit_body(date, value)?;
            let data =
                update_point(&client, &nest, &key, series_id.as_deref(), &point_id, &body).await?;
            render::render_object(&data, output, |d| {
                println!(
                    "Point {} is now {} = {}. The tracked field re-syncs to the newest point at or before now.",
                    render::sanitize(d.get("id").and_then(Value::as_str).unwrap_or(&point_id)),
                    render::sanitize(d.get("date").and_then(Value::as_str).unwrap_or("")),
                    d.get("value").map(Value::to_string).unwrap_or_default()
                );
            })?;
        }
        SeriesCmd::Delete {
            nest,
            key,
            point_id,
            series_id,
        } => {
            safety::enforce_read_only(g.read_only, "nests series delete")?;
            safety::confirm_destructive(
                &format!("Delete point '{point_id}' from series '{key}' on '{nest}'?"),
                g.yes,
            )?;
            let data = delete_point(&client, &nest, &key, series_id.as_deref(), &point_id).await?;
            render::render_object(&data, output, |_| {
                println!(
                    "Deleted point {}. The tracked field re-syncs to the newest point left at or before now.",
                    render::sanitize(&point_id)
                );
            })?;
        }
    }
    Ok(())
}
