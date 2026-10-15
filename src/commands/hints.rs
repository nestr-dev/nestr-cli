use anyhow::Result;
use clap::Args;
use colored::Colorize as _;
use serde_json::Value;

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::nests::SEVERITIES;
use crate::commands::{resolve_client, GlobalArgs};
use crate::config::OutputFormat;
use crate::render::{self, print_json};

#[derive(Args)]
pub struct HintsArgs {
    /// Circle or other nest to roll up (default: the active workspace).
    pub nest_id: Option<String>,
    /// Count only these hint types (comma-separated).
    #[arg(
        long = "type",
        alias = "hint-types",
        value_name = "TYPE",
        value_delimiter = ','
    )]
    pub types: Vec<String>,
    /// Drop hint types below this severity (`warning` is the "needs attention" floor).
    #[arg(long, value_parser = SEVERITIES)]
    pub min_severity: Option<String>,
    /// Example nests per type (default 10, max 100, 0 for counts only).
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=100))]
    pub sample_size: Option<u32>,
}

/// `{scope, results, notComputed}` for everything under `nest_id`.
pub async fn fetch_rollup(
    client: &NestrClient,
    nest_id: &str,
    types: &[String],
    min_severity: Option<&str>,
    sample_size: Option<u32>,
) -> crate::error::Result<Value> {
    let types = types.join(",");
    let sample = sample_size.map(|n| n.to_string());
    let mut params: Vec<(&str, &str)> = Vec::new();
    if !types.is_empty() {
        params.push(("hintTypes", &types));
    }
    if let Some(m) = min_severity {
        params.push(("minSeverity", m));
    }
    if let Some(s) = &sample {
        params.push(("sampleSize", s));
    }
    let raw: Value = client
        .get(&format!("/nests/{nest_id}/hints"), &params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The sample's titles, with how many more the count holds.
pub fn sample_cell(result: &Value) -> String {
    let sample = result
        .get("sample")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let titles: Vec<String> = sample
        .iter()
        .map(|n| render::clean_text(n.get("title").and_then(Value::as_str).unwrap_or("")))
        .collect();
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(0);
    let rest = count.saturating_sub(titles.len() as u64);
    match (titles.is_empty(), rest) {
        (true, _) => String::new(),
        (false, 0) => titles.join(", "),
        (false, n) => format!("{} (+{n} more)", titles.join(", ")),
    }
}

pub async fn run(args: HintsArgs, g: &GlobalArgs) -> Result<()> {
    let (cfg, client) = resolve_client(g).await?;
    let nest_id = match &args.nest_id {
        Some(id) => id.clone(),
        None => cfg.require_workspace()?.to_string(),
    };
    let data = fetch_rollup(
        &client,
        &nest_id,
        &args.types,
        args.min_severity.as_deref(),
        args.sample_size,
    )
    .await?;
    if cfg.output == OutputFormat::Json {
        return print_json(&data);
    }
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if results.is_empty() {
        render::print_no_results("No hints counted in this scope.");
    } else {
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                let s =
                    |k: &str| render::clean_text(r.get(k).and_then(Value::as_str).unwrap_or(""));
                vec![
                    s("type"),
                    s("severity"),
                    r.get("count")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .to_string(),
                    sample_cell(r),
                ]
            })
            .collect();
        println!(
            "{}",
            render::format_table(&["TYPE", "SEVERITY", "NESTS", "SAMPLE"], rows)
        );
        println!(
            "{}",
            "Counts overlap (one nest can carry several hints); don't sum them.".dimmed()
        );
    }
    let not_computed: Vec<String> = data
        .get("notComputed")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(|k| render::clean_text(k)).collect())
        .unwrap_or_default();
    if !not_computed.is_empty() {
        println!(
            "{}",
            format!(
                "not computed (unknown, not zero; -o json says why): {}",
                not_computed.join(", ")
            )
            .dimmed()
        );
    }
    Ok(())
}
