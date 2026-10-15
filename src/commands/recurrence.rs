//! `nests recurrence`: a nest's repeat rule and its occurrences.
//!
//! Occurrences past the next one are virtual: the rule produces their instant and
//! no nest exists for them, so search and `nests children` cannot see them. Every
//! occurrence, virtual or not, is addressed by its `instant` (epoch milliseconds)
//! as `GET nests/:id/recurrence` lists it, and every route takes the id of the
//! series or of any occurrence of it.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::nests::{apply_schedule, parse_field_args, resolve_assignees, update_body};
use crate::commands::GlobalArgs;
use crate::config::OutputFormat;
use crate::error::NestrError;
use crate::{render, safety, validation};

#[derive(Subcommand)]
pub enum RecurrenceCmd {
    /// Make a nest repeat on an RFC 5545 RRULE, e.g. FREQ=WEEKLY;BYDAY=MO,WE;COUNT=10.
    /// The rule expands from the nest's start (its due when it has no start), so set
    /// a date first. Creates no nests: occurrences stay virtual until one is touched.
    Set {
        /// The series, or any occurrence of it.
        id: String,
        /// The RRULE, without the `RRULE:` prefix.
        rrule: String,
    },
    /// Stop repeating. Occurrences already materialized are kept, detached from the
    /// series. To delete the series and its history, use `delete-series`.
    Remove {
        /// The series, or any occurrence of it.
        id: String,
    },
    /// List a series' occurrences, virtual and materialized, with the `instant` each
    /// is addressed by. The only way to see virtual ones.
    #[command(visible_alias = "list")]
    Occurrences {
        /// The series, or any occurrence of it.
        id: String,
        /// Walk back through history (most recent first) instead of forward.
        #[arg(long)]
        past: bool,
        /// Start after this instant (epoch milliseconds); defaults to now. Pass the
        /// cursor the previous page printed.
        #[arg(long)]
        cursor: Option<String>,
        /// Occurrences per page (server default 10, capped at 50).
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Skip one occurrence; the series carries on. With --following, delete it and
    /// every later one instead, keeping the history before it.
    Skip {
        /// The series, or any occurrence of it.
        id: String,
        /// The occurrence's `instant` from `occurrences` (epoch milliseconds).
        instant: String,
        /// Delete this occurrence and every later one (ends the series here).
        #[arg(long)]
        following: bool,
    },
    /// Edit one occurrence, materializing it first if it is still virtual. With no
    /// flags it only materializes. The rule and other occurrences are untouched.
    Edit(Box<EditArgs>),
    /// Delete the whole series: the series nest and every occurrence, past ones
    /// included. To stop repeating and keep the nests, use `remove`.
    DeleteSeries {
        /// The series, or any occurrence of it.
        id: String,
    },
}

#[derive(Args)]
pub struct EditArgs {
    /// The series, or any occurrence of it.
    pub id: String,
    /// The occurrence's `instant` from `occurrences` (epoch milliseconds).
    pub instant: String,
    /// This occurrence's name.
    #[arg(long)]
    pub title: Option<String>,
    /// One-line *why*; body text goes in --description.
    #[arg(long)]
    pub purpose: Option<String>,
    /// This occurrence's body (Markdown).
    #[arg(long)]
    pub description: Option<String>,
    /// Move this occurrence under a new parent id.
    #[arg(long)]
    pub parent: Option<String>,
    /// Due date (ISO 8601 or yyyy-mm-dd); with a start, the end of the occurrence.
    #[arg(long)]
    pub due: Option<String>,
    /// Move this occurrence: an ISO 8601 date-time, or yyyy-mm-dd for a whole day.
    #[arg(long)]
    pub start: Option<String>,
    /// Whole calendar days (true) or instants (false). Bare `--all-day` means true.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub all_day: Option<bool>,
    /// Mark this occurrence done (or not).
    #[arg(long)]
    pub completed: Option<bool>,
    /// Replace this occurrence's assignees (repeatable); `me` for yourself.
    #[arg(long = "assignee")]
    pub assignees: Vec<String>,
    /// Replace this occurrence's label set (repeatable).
    #[arg(long = "label")]
    pub labels: Vec<String>,
    /// Set a field as KEY=VALUE (repeatable), as on `nests update`.
    #[arg(long = "field")]
    pub fields: Vec<String>,
}

/// `PATCH nests/:id/recurrence`: a rule string sets it, `None` removes it.
pub async fn set_recurrence(
    client: &NestrClient,
    id: &str,
    rrule: Option<&str>,
) -> crate::error::Result<Value> {
    let body = serde_json::json!({ "rrule": rrule });
    let raw: Value = client
        .patch(&format!("/nests/{id}/recurrence"), &body)
        .await?;
    Ok(unwrap_data(raw).0)
}

/// `GET nests/:id/recurrence`: one cursor-paged page of occurrences.
pub async fn list_occurrences(
    client: &NestrClient,
    id: &str,
    params: &[(&str, &str)],
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/nests/{id}/recurrence"), params)
        .await?;
    Ok(unwrap_data(raw).0)
}

/// `DELETE nests/:id/recurrence/:instant`, with `scope=following` when asked.
pub async fn skip_occurrence(
    client: &NestrClient,
    id: &str,
    instant: i64,
    following: bool,
) -> crate::error::Result<Value> {
    let params: &[(&str, &str)] = if following {
        &[("scope", "following")]
    } else {
        &[]
    };
    let raw: Value = client
        .delete_with_params(&format!("/nests/{id}/recurrence/{instant}"), params)
        .await?;
    Ok(unwrap_data(raw).0)
}

/// `PATCH nests/:id/recurrence/:instant`. Answers with the occurrence's nest,
/// sometimes bare and sometimes enveloped; a nest has its own `data` store, so
/// the envelope is told apart by the absence of `_id`.
pub async fn edit_occurrence(
    client: &NestrClient,
    id: &str,
    instant: i64,
    body: &Value,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .patch(&format!("/nests/{id}/recurrence/{instant}"), body)
        .await?;
    if raw.get("_id").is_some() {
        return Ok(raw);
    }
    Ok(unwrap_data(raw).0)
}

/// `DELETE nests/:id/recurrence`: the series and every live occurrence.
pub async fn delete_series(client: &NestrClient, id: &str) -> crate::error::Result<Value> {
    let raw: Value = client.delete(&format!("/nests/{id}/recurrence")).await?;
    Ok(unwrap_data(raw).0)
}

/// An occurrence's instant as the API takes it: integer epoch milliseconds.
/// A date is refused rather than converted, because the instant must match the
/// one the rule produces exactly and the listing already prints it.
pub fn parse_instant(raw: &str) -> crate::error::Result<i64> {
    raw.trim().parse::<i64>().map_err(|_| {
        NestrError::Validation(format!(
            "instant must be epoch milliseconds, got '{raw}'. Copy the INSTANT column \
             from `nestr nests recurrence occurrences <id>`."
        ))
    })
}

/// The edit refusals the route answers before it materializes anything: an
/// instant the series lacks or has skipped, or a nest with no rule at all.
fn refused_before_materializing(e: &NestrError) -> bool {
    let msg = e.to_string();
    ["has been skipped", "no occurrence at", "no repeat rule"]
        .iter()
        .any(|s| msg.contains(s))
}

fn restore_note(id: &str) -> String {
    format!(
        "Restore nest {id} in the Nestr app to undo; occurrences deleted with it are restored separately."
    )
}

/// One line saying what a skip or a cut did, from the keys the server returned.
pub fn skip_message(data: &Value) -> String {
    let at = data
        .get("instant")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let restore = data.get("restoreId").and_then(Value::as_str);
    if data.get("deleted").and_then(Value::as_bool) == Some(true) {
        let series = data.get("seriesId").and_then(Value::as_str);
        let head = if restore.is_some() && restore == series {
            format!("The cut at {at} was the first occurrence, so the whole series was deleted.")
        } else {
            format!(
                "Deleted the occurrence at {at} and every later one. The history before it is kept, under a rule that now ends there."
            )
        };
        return match restore {
            Some(r) => format!("{head} {}", restore_note(r)),
            None => head,
        };
    }
    match restore {
        Some(r) => format!(
            "Skipped the occurrence at {at}; it existed as a nest and was deleted. {}",
            restore_note(r)
        ),
        None => format!(
            "Skipped the occurrence at {at}. The series continues; the instant stays listed as skipped."
        ),
    }
}

pub async fn run(cmd: RecurrenceCmd, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = crate::commands::resolve_client(g).await?;
    let output = cfg.output;
    match cmd {
        RecurrenceCmd::Set { id, rrule } => {
            safety::enforce_read_only(g.read_only, "nests recurrence set")?;
            let rrule = rrule.trim();
            let rrule = rrule.strip_prefix("RRULE:").unwrap_or(rrule);
            let data = set_recurrence(&client, &id, Some(rrule)).await?;
            render::render_object(&data, output, |d| {
                let rule = d.get("rrule").and_then(Value::as_str).unwrap_or(rrule);
                println!(
                    "Recurrence set to {}. Occurrences stay virtual until one is touched; list them with `nestr nests recurrence occurrences {id}`.",
                    render::clean_text(rule)
                );
            })?;
        }
        RecurrenceCmd::Remove { id } => {
            safety::enforce_read_only(g.read_only, "nests recurrence remove")?;
            let data = set_recurrence(&client, &id, None).await?;
            render::render_object(&data, output, |_| {
                println!(
                    "Recurrence removed. Future occurrences stop; anything already materialized is kept, detached from the series."
                );
            })?;
        }
        RecurrenceCmd::Occurrences {
            id,
            past,
            cursor,
            limit,
        } => {
            let limit = limit.map(|n| n.to_string());
            let mut params: Vec<(&str, &str)> = Vec::new();
            if past {
                params.push(("direction", "past"));
            }
            if let Some(c) = &cursor {
                params.push(("cursor", c));
            }
            if let Some(l) = &limit {
                params.push(("limit", l));
            }
            let data = list_occurrences(&client, &id, &params).await?;
            match output {
                OutputFormat::Json => render::print_json(&data)?,
                OutputFormat::Text => println!("{}", render::occurrences_text(&data)),
            }
        }
        RecurrenceCmd::Skip {
            id,
            instant,
            following,
        } => {
            safety::enforce_read_only(g.read_only, "nests recurrence skip")?;
            let at = parse_instant(&instant)?;
            let prompt = if following {
                format!("Delete the occurrence at {at} of '{id}' and every later one?")
            } else {
                format!("Skip the occurrence at {at} of '{id}'?")
            };
            safety::confirm_destructive(&prompt, g.yes)?;
            let data = skip_occurrence(&client, &id, at, following).await?;
            render::render_object(&data, output, |d| println!("{}", skip_message(d)))?;
        }
        RecurrenceCmd::Edit(args) => {
            let EditArgs {
                id,
                instant,
                title,
                purpose,
                description,
                parent,
                due,
                start,
                all_day,
                completed,
                assignees,
                labels,
                fields,
            } = *args;
            safety::enforce_read_only(g.read_only, "nests recurrence edit")?;
            let at = parse_instant(&instant)?;
            if !labels.is_empty() {
                validation::validate_prime_labels(&labels)?;
            }
            let field_values = parse_field_args(&fields)?;
            let users = resolve_assignees(&client, &assignees).await?;
            let mut body = update_body(
                title,
                purpose,
                description,
                due,
                completed,
                parent,
                &labels,
                &users,
                &field_values,
            );
            apply_schedule(&mut body, start, all_day, false);
            let nest = match edit_occurrence(&client, &id, at, &body).await {
                Ok(n) => n,
                // The route materializes before it applies the body, so a refused
                // edit can still have created the occurrence's nest. Refusals about
                // the instant itself come before that and write nothing.
                Err(e @ (NestrError::Validation(_) | NestrError::Permission(_)))
                    if !refused_before_materializing(&e) =>
                {
                    return Err(anyhow::anyhow!(
                        "{e}\nThe occurrence may already be materialized even though the edit was refused: `nestr nests recurrence occurrences {id}` shows its nest id."
                    ));
                }
                Err(e) => return Err(e.into()),
            };
            if output == OutputFormat::Text {
                if let Some(nid) = nest.get("_id").and_then(Value::as_str) {
                    eprintln!(
                        "Only this occurrence changed. It is nest {nid} now: use that id with every other nests command."
                    );
                }
            }
            render::output_nest_detail(&nest, &cfg.host, output)?;
        }
        RecurrenceCmd::DeleteSeries { id } => {
            safety::enforce_read_only(g.read_only, "nests recurrence delete-series")?;
            safety::confirm_destructive(
                &format!("Delete the whole series of '{id}', past occurrences included?"),
                g.yes,
            )?;
            let data = delete_series(&client, &id).await?;
            render::render_object(&data, output, |d| {
                let restore = d
                    .get("restoreId")
                    .and_then(Value::as_str)
                    .map(restore_note)
                    .unwrap_or_default();
                println!(
                    "Series deleted: the series nest and every occurrence, past ones included. {restore}"
                );
            })?;
        }
    }
    Ok(())
}
