use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;

use crate::config::OutputFormat;

pub fn format_json(rows: &[Value]) -> Result<String> {
    Ok(serde_json::to_string_pretty(rows)?)
}

/// Pretty JSON; a single element renders as a bare object (nicer for `get`).
pub fn format_json_auto(rows: &[Value]) -> Result<String> {
    if rows.len() == 1 {
        Ok(serde_json::to_string_pretty(&rows[0])?)
    } else {
        Ok(serde_json::to_string_pretty(rows)?)
    }
}

pub fn format_table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut builder = Builder::default();
    builder.push_record(headers.iter().map(|h| h.to_string()));
    for row in rows {
        builder.push_record(row);
    }
    builder.build().to_string()
}

pub fn print_no_results(msg: &str) {
    println!("{}", msg.yellow());
}

/// Render a single object either as JSON or via a caller-supplied text summary.
pub fn render_object(
    value: &Value,
    output: OutputFormat,
    text_summary: impl Fn(&Value),
) -> Result<()> {
    match output {
        OutputFormat::Json => println!("{}", format_json_auto(std::slice::from_ref(value))?),
        OutputFormat::Text => text_summary(value),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_is_pretty_array() {
        let out = format_json(&[json!({"a":1})]).unwrap();
        assert!(out.contains("\"a\": 1"));
        assert!(out.starts_with('['));
    }

    #[test]
    fn table_has_headers_and_rows() {
        let out = format_table(&["NAME", "HOST"], vec![vec!["prod".into(), "x".into()]]);
        assert!(out.contains("NAME"));
        assert!(out.contains("prod"));
    }

    #[test]
    fn json_auto_unwraps_single_but_keeps_multiple() {
        let single = format_json_auto(&[json!({"a":1})]).unwrap();
        assert!(
            single.starts_with('{'),
            "single element should be a bare object, got: {single}"
        );

        let multiple = format_json_auto(&[json!({"a":1}), json!({"b":2})]).unwrap();
        assert!(
            multiple.starts_with('['),
            "multiple elements should be a JSON array, got: {multiple}"
        );
    }
}
