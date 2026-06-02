use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;

use crate::config::OutputFormat;
use crate::views::{AppView, CompactNest, GroupView, RoleView, UserView};

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

/// Pretty-print a raw value as-is (array or object) — the `-o json` path.
pub fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Standard compact table for any list of Nest-shaped objects.
pub fn nest_table(nests: &[CompactNest]) -> String {
    let rows: Vec<Vec<String>> = nests
        .iter()
        .map(|n| {
            vec![
                n.id.clone(),
                n.title.clone(),
                n.due.clone().unwrap_or_default(),
                match n.completed {
                    Some(true) => "✓".to_string(),
                    _ => String::new(),
                },
                n.labels_str(),
            ]
        })
        .collect();
    format_table(&["ID", "TITLE", "DUE", "DONE", "LABELS"], rows)
}

/// Render a list of Nest-shaped objects: raw JSON, or a compact table + footers.
pub fn output_nests(data: &Value, meta: Option<&Value>, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let nests: Vec<CompactNest> = serde_json::from_value(data.clone()).unwrap_or_default();
            if nests.is_empty() {
                print_no_results("No results.");
                return Ok(());
            }
            println!("{}", nest_table(&nests));
            if let Some(f) = pagination_footer(meta) {
                println!("{f}");
            }
            if let Some(h) = hint_line(data) {
                println!("{h}");
            }
        }
    }
    Ok(())
}

/// Render a single Nest-shaped object as a detail block (or raw JSON).
pub fn output_nest_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let n: CompactNest = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("{}  [{}]", n.title, n.id);
            if let Some(p) = n.purpose.as_deref().filter(|s| !s.is_empty()) {
                println!("purpose: {p}");
            }
            if !n.labels.is_empty() {
                println!("labels: {}", n.labels_str());
            }
            if let Some(d) = &n.due {
                println!("due: {d}");
            }
            if let Some(c) = n.completed {
                println!("completed: {c}");
            }
            if let Some(h) = hint_line(data) {
                println!("{h}");
            }
        }
    }
    Ok(())
}

/// Build a `page x/y · N total · --page n for more` footer from a `meta` object.
pub fn pagination_footer(meta: Option<&Value>) -> Option<String> {
    let m = meta?;
    let page = m.get("page").and_then(Value::as_u64)?;
    let total_pages = m.get("total_pages").and_then(Value::as_u64).unwrap_or(page);
    let mut s = format!("page {page}/{total_pages}");
    if let Some(total) = m.get("total").and_then(Value::as_u64) {
        s.push_str(&format!(" · {total} total"));
    }
    if page < total_pages {
        s.push_str(&format!(" · --page {} for more", page + 1));
    }
    Some(s.dimmed().to_string())
}

/// Best-effort: surface the first `hints[].url` found anywhere in the payload.
/// Phase 1 does not request hints by default, so this is usually dormant.
pub fn hint_line(data: &Value) -> Option<String> {
    fn find(v: &Value) -> Option<String> {
        match v {
            Value::Array(a) => a.iter().find_map(find),
            Value::Object(o) => {
                if let Some(Value::Array(hints)) = o.get("hints") {
                    for h in hints {
                        if let Some(u) = h.get("url").and_then(Value::as_str) {
                            return Some(u.to_string());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
    find(data).map(|u| format!("next: {u}").dimmed().to_string())
}

/// Compact table for roles/circles. ACC/DOM are counts; full titles live in `role_detail`.
pub fn role_table(roles: &[RoleView]) -> String {
    let rows: Vec<Vec<String>> = roles
        .iter()
        .map(|r| {
            vec![
                r.id.clone(),
                r.title.clone(),
                r.acc_titles().len().to_string(),
                r.domain_titles().len().to_string(),
                r.labels_str(),
            ]
        })
        .collect();
    format_table(&["ID", "TITLE", "ACC", "DOM", "LABELS"], rows)
}

/// Render a list of roles/circles: raw JSON, or a compact table + footer.
pub fn output_roles(data: &Value, meta: Option<&Value>, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let roles: Vec<RoleView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if roles.is_empty() {
                print_no_results("No results.");
                return Ok(());
            }
            println!("{}", role_table(&roles));
            if let Some(f) = pagination_footer(meta) {
                println!("{f}");
            }
        }
    }
    Ok(())
}

/// Detail block for a single role/circle, listing full accountability/domain titles.
pub fn role_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let r: RoleView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("{}  [{}]", r.title, r.id);
            if let Some(p) = r.purpose.as_deref().filter(|s| !s.is_empty()) {
                println!("purpose: {p}");
            }
            if let Some(pid) = &r.parent_id {
                println!("parent: {pid}");
            }
            let acc = r.acc_titles();
            if !acc.is_empty() {
                println!("accountabilities:");
                for a in acc {
                    println!("  - {a}");
                }
            }
            let dom = r.domain_titles();
            if !dom.is_empty() {
                println!("domains:");
                for d in dom {
                    println!("  - {d}");
                }
            }
            if !r.labels.is_empty() {
                println!("labels: {}", r.labels_str());
            }
        }
    }
    Ok(())
}

pub fn user_table(users: &[UserView]) -> String {
    let rows: Vec<Vec<String>> = users
        .iter()
        .map(|u| {
            vec![
                u.id.clone(),
                u.username.clone().unwrap_or_default(),
                u.full_name(),
                if u.bot == Some(true) { "bot" } else { "" }.to_string(),
            ]
        })
        .collect();
    format_table(&["ID", "USERNAME", "NAME", "BOT"], rows)
}

pub fn group_table(groups: &[GroupView]) -> String {
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|g| vec![g.id.clone(), g.name.clone().unwrap_or_default()])
        .collect();
    format_table(&["ID", "NAME"], rows)
}

pub fn app_table(apps: &[AppView]) -> String {
    let rows: Vec<Vec<String>> = apps
        .iter()
        .map(|a| {
            vec![
                a.id.clone(),
                a.title.clone().unwrap_or_default(),
                if a.enabled { "✓" } else { "" }.to_string(),
            ]
        })
        .collect();
    format_table(&["ID", "TITLE", "ENABLED"], rows)
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

    #[test]
    fn nest_table_renders_id_and_title() {
        use crate::views::CompactNest;
        let n: CompactNest =
            serde_json::from_value(json!({"_id":"abc","title":"Do thing","labels":["now"]}))
                .unwrap();
        let out = nest_table(&[n]);
        assert!(out.contains("abc"));
        assert!(out.contains("Do thing"));
        assert!(out.contains("now"));
    }

    #[test]
    fn pagination_footer_present_only_when_more_pages() {
        let meta = json!({"page":1,"total_pages":3,"total":57});
        let f = pagination_footer(Some(&meta)).unwrap();
        assert!(f.contains("page 1/3"));
        assert!(f.contains("57 total"));
        assert!(f.contains("--page 2"));
        // Last page → no "for more" hint, but still a footer.
        let last = json!({"page":3,"total_pages":3,"total":57});
        assert!(!pagination_footer(Some(&last)).unwrap().contains("for more"));
        // No meta → no footer.
        assert!(pagination_footer(None).is_none());
    }

    #[test]
    fn hint_line_none_when_absent() {
        assert!(hint_line(&json!([{"_id":"a"}])).is_none());
    }

    #[test]
    fn hint_line_extracts_first_url() {
        let data = json!([{"_id":"a","hints":[{"url":"/nests/a/children?search=x"}]}]);
        let line = hint_line(&data).unwrap();
        assert!(line.contains("/nests/a/children?search=x"));
    }

    #[test]
    fn role_table_shows_counts_and_title() {
        use crate::views::RoleView;
        let r: RoleView = serde_json::from_value(json!({
            "_id": "r1", "title": "Lead", "labels": ["role"],
            "accountabilities": [{"title": "x"}, {"title": "y"}], "domains": [{"title": "z"}]
        }))
        .unwrap();
        let out = role_table(&[r]);
        assert!(out.contains("Lead") && out.contains("r1"));
        assert!(out.contains('2') && out.contains('1')); // 2 acc, 1 dom
    }

    #[test]
    fn user_and_group_and_app_tables_render() {
        use crate::views::{AppView, GroupView, UserView};
        let u: UserView = serde_json::from_value(
            json!({"_id":"u1","username":"a@b.c","profile":{"fullName":"A B"}}),
        )
        .unwrap();
        assert!(user_table(&[u]).contains("a@b.c"));
        let g: GroupView = serde_json::from_value(json!({"_id":"g1","name":"leads"})).unwrap();
        assert!(group_table(&[g]).contains("leads"));
        let a: AppView =
            serde_json::from_value(json!({"_id":"okr","title":"OKR","enabled":true})).unwrap();
        assert!(app_table(&[a]).contains("OKR"));
    }
}
