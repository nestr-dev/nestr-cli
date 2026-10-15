//! Advisory notices the API puts beside `data` in a response envelope.
//!
//! A write that ignored a body key still answers 200, a search that dropped an
//! unknown operator still returns rows, and `nests children --search` quietly adds
//! `depth:1`. The API says so in `warnings`, `searchWarnings`, `appliedDefaults` and
//! a top-level `hints` list, and `unwrap_data` throws all of those away. The client
//! pulls them out of every response (`NestrClient::request`) and prints them to
//! stderr, so stdout keeps carrying only the unwrapped data in both output modes:
//!
//! - text: one line per notice, `warning: …` / `note: …` / `hint: …`.
//! - json: one compact JSON object per response holding just those keys, verbatim.

use colored::Colorize;
use serde_json::{Map, Value};

use crate::config::OutputFormat;
use crate::render::clean_text;

/// The envelope keys that carry notices, in the order they print.
pub const NOTICE_KEYS: &[&str] = &["warnings", "searchWarnings", "appliedDefaults", "hints"];

/// The non-empty notice keys of a response envelope, or `None`. Only an object with
/// a `status` key counts as an envelope, so a bare nest that happens to have a
/// `hints` field is never mistaken for one.
pub fn extract(body: &Value) -> Option<Map<String, Value>> {
    let obj = body.as_object().filter(|o| o.contains_key("status"))?;
    let found: Map<String, Value> = NOTICE_KEYS
        .iter()
        .filter_map(|k| {
            let v = obj.get(*k)?;
            let empty = v.is_null() || v.as_array().is_some_and(|a| a.is_empty());
            (!empty).then(|| (k.to_string(), v.clone()))
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

/// The human-readable lines for a notice map (text mode).
pub fn text_lines(notices: &Map<String, Value>) -> Vec<String> {
    let mut lines = Vec::new();
    for key in NOTICE_KEYS {
        let Some(v) = notices.get(*key) else { continue };
        let items: Vec<&Value> = match v {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for item in items {
            let line = match *key {
                "warnings" | "searchWarnings" => message_of(item).map(|m| format!("warning: {m}")),
                "appliedDefaults" => applied_default(item).map(|m| format!("note: {m}")),
                _ => item
                    .get("label")
                    .and_then(Value::as_str)
                    .map(|l| format!("hint: {l}")),
            };
            if let Some(l) = line {
                lines.push(clean_text(&l));
            }
        }
    }
    lines
}

/// Print one response's notices to stderr in the given output mode.
pub fn emit(notices: &Map<String, Value>, output: OutputFormat) {
    match output {
        OutputFormat::Json => {
            if let Ok(s) = serde_json::to_string(notices) {
                eprintln!("{s}");
            }
        }
        OutputFormat::Text => {
            for line in text_lines(notices) {
                eprintln!("{}", line.yellow());
            }
        }
    }
}

fn message_of(item: &Value) -> Option<String> {
    match item {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("message").and_then(Value::as_str).map(String::from),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

fn applied_default(item: &Value) -> Option<String> {
    let op = item.get("operator").and_then(Value::as_str)?;
    let mut s = format!("added {op}");
    for k in ["reason", "override"] {
        if let Some(t) = item.get(k).and_then(Value::as_str) {
            s.push_str(". ");
            s.push_str(t.trim_end_matches('.'));
        }
    }
    s.push('.');
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_needs_an_envelope() {
        assert!(extract(&json!({"_id":"n1","hints":[{"label":"x"}]})).is_none());
        assert!(extract(&json!([{"warnings":["x"]}])).is_none());
    }

    #[test]
    fn extract_skips_empty_and_unrelated_keys() {
        let n = extract(&json!({"status":"success","data":{},"warnings":[],"meta":{}}));
        assert!(n.is_none());
        let n = extract(&json!({"status":"success","data":{},"warnings":["w"]})).unwrap();
        assert_eq!(n.keys().collect::<Vec<_>>(), vec!["warnings"]);
    }

    #[test]
    fn text_lines_cover_every_kind() {
        let n = extract(&json!({
            "status": "success",
            "warnings": ["Ignored unsupported parameter: foo. It was not saved."],
            "searchWarnings": [{"type":"unknown_label","message":"Unknown label 'x' in 'label:x'. This filter was ignored."}],
            "appliedDefaults": [{"operator":"depth:1","reason":"Scoped to direct children.","override":"Set a depth yourself."}],
            "hints": [{"type":"unread_posts","label":"2 posts you have not read"}],
            "data": []
        }))
        .unwrap();
        assert_eq!(
            text_lines(&n),
            vec![
                "warning: Ignored unsupported parameter: foo. It was not saved.",
                "warning: Unknown label 'x' in 'label:x'. This filter was ignored.",
                "note: added depth:1. Scoped to direct children. Set a depth yourself.",
                "hint: 2 posts you have not read",
            ]
        );
    }

    #[test]
    fn text_lines_strip_control_characters() {
        let n = extract(&json!({"status":"success","warnings":["bad\u{1b}[31m key"]})).unwrap();
        assert!(!text_lines(&n)[0].contains('\u{1b}'));
    }
}
