use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;

use crate::config::OutputFormat;
use crate::views::{
    AppView, ChangeView, ChildView, CompactNest, GroupView, InsightView, LinkView, PartView,
    RoleView, StatusView, TensionView, UserView, WebhookView,
};

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
        builder.push_record(row.iter().map(|c| clean_text(c)));
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
pub fn output_nests(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    supports_page: bool,
) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let nests: Vec<CompactNest> = serde_json::from_value(data.clone()).unwrap_or_default();
            if nests.is_empty() {
                print_no_results("No results.");
                return Ok(());
            }
            println!("{}", nest_table(&nests));
            if let Some(f) = pagination_footer(meta, supports_page) {
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
            println!("{}  [{}]", clean_text(&n.title), n.id);
            if let Some(p) = n.purpose.as_deref().filter(|s| !s.is_empty()) {
                println!("purpose: {}", clean_text(p));
            }
            if !n.labels.is_empty() {
                println!("labels: {}", clean_text(&n.labels_str()));
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
/// `supports_page` gates the `--page n for more` hint for commands without a
/// `--page` flag (the page/total counts still print). (COR-11)
pub fn pagination_footer(meta: Option<&Value>, supports_page: bool) -> Option<String> {
    let m = meta?;
    let page = m.get("page").and_then(Value::as_u64)?;
    let total_pages = m.get("total_pages").and_then(Value::as_u64).unwrap_or(page);
    let mut s = format!("page {page}/{total_pages}");
    if let Some(total) = m.get("total").and_then(Value::as_u64) {
        s.push_str(&format!(" · {total} total"));
    }
    if supports_page && page < total_pages {
        s.push_str(&format!(" · --page {} for more", page + 1));
    }
    Some(s.dimmed().to_string())
}

/// Footer for skip/limit-paginated lists that return no `meta` (notifications).
/// Prints when the page is full (rows == limit), pointing at the next `--skip`.
/// `limit` defaults to the server's 50 when not given. (COR-10)
pub fn skip_limit_footer(skip: u32, limit: Option<u32>, rows: usize) -> Option<String> {
    let limit = limit.unwrap_or(50);
    if rows as u32 == limit && limit > 0 {
        let next = skip + limit;
        Some(
            format!(
                "showing {}-{} · --skip {next} for more",
                skip + 1,
                skip + rows as u32
            )
            .dimmed()
            .to_string(),
        )
    } else {
        None
    }
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
    find(data).map(|u| format!("next: {}", clean_text(&u)).dimmed().to_string())
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
pub fn output_roles(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    supports_page: bool,
) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let roles: Vec<RoleView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if roles.is_empty() {
                print_no_results("No results.");
                return Ok(());
            }
            println!("{}", role_table(&roles));
            if let Some(f) = pagination_footer(meta, supports_page) {
                println!("{f}");
            }
        }
    }
    Ok(())
}

/// Drop ANSI/terminal control sequences from an API-derived string before it is
/// printed to a terminal. Keeps `\n` and `\t`; removes ESC, BEL, CSI/OSC bytes,
/// bare CR, etc. Guards against OSC-52 clipboard writes, title spoofing, and
/// cursor/erase tricks authored by other workspace members (SEC-1).
fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// The uniform cleaner for every API string shown in text mode. Mirrors the web
/// app's `html2plaintext`: strip the HTML tags, then decode the full HTML5 entity
/// set (`&nbsp;`, `&amp;`, `&#8217;`, …) via `htmlize`. Kept single-line — table
/// cells can't hold the `<p>`/`<br>` breaks `html2plaintext` emits, so they collapse
/// to spaces — and re-sanitized *after* decoding so an entity-encoded control char
/// (e.g. `&#27;`) can't smuggle a terminal escape back in.
pub fn clean_text(s: &str) -> String {
    let stripped = strip_html(&sanitize(s));
    let decoded = htmlize::unescape(stripped.as_str());
    sanitize(&decoded)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip HTML tags and collapse whitespace from rich-text titles for terminal display.
/// Accountability/domain titles can be HTML that the server's `cleanText` doesn't touch.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Detail block for a single role/circle, listing full accountability/domain titles.
pub fn role_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let r: RoleView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("{}  [{}]", clean_text(&r.title), r.id);
            if let Some(p) = r.purpose.as_deref().filter(|s| !s.is_empty()) {
                println!("purpose: {}", clean_text(p));
            }
            if let Some(pid) = &r.parent_id {
                println!("parent: {pid}");
            }
            let acc = r.acc_titles();
            if !acc.is_empty() {
                println!("accountabilities:");
                for a in acc {
                    println!("  - {}", clean_text(&a));
                }
            }
            let dom = r.domain_titles();
            if !dom.is_empty() {
                println!("domains:");
                for d in dom {
                    println!("  - {}", clean_text(&d));
                }
            }
            if !r.labels.is_empty() {
                println!("labels: {}", clean_text(&r.labels_str()));
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

/// Compact table for tensions.
pub fn tension_table(tensions: &[TensionView]) -> String {
    let rows: Vec<Vec<String>> = tensions
        .iter()
        .map(|t| {
            vec![
                t.id.clone(),
                t.title.clone(),
                t.status.clone().unwrap_or_default(),
                crate::views::join_labels(&t.labels),
            ]
        })
        .collect();
    format_table(&["ID", "TITLE", "STATUS", "LABELS"], rows)
}

pub fn output_tensions(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    supports_page: bool,
) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let ts: Vec<TensionView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if ts.is_empty() {
                print_no_results("No tensions.");
                return Ok(());
            }
            println!("{}", tension_table(&ts));
            if let Some(f) = pagination_footer(meta, supports_page) {
                println!("{f}");
            }
        }
    }
    Ok(())
}

pub fn tension_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let t: TensionView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("{}  [{}]", clean_text(&t.title), t.id);
            if let Some(s) = &t.status {
                println!("status: {}", clean_text(s));
            }
            if let Some(d) = t.description.as_deref().filter(|s| !s.is_empty()) {
                println!("description: {}", clean_text(d));
            }
            for (label, key) in [("feeling", "tension.feeling"), ("needs", "tension.needs")] {
                if let Some(v) = data
                    .get("fields")
                    .and_then(|f| f.get(key))
                    .and_then(Value::as_str)
                {
                    if !v.is_empty() {
                        println!("{label}: {}", clean_text(v));
                    }
                }
            }
            if !t.labels.is_empty() {
                println!(
                    "labels: {}",
                    clean_text(&crate::views::join_labels(&t.labels))
                );
            }
        }
    }
    Ok(())
}

/// One row per part, summarising its primary proposal item (`items[0]`).
pub fn output_parts(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let parts: Vec<PartView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if parts.is_empty() {
                print_no_results("No parts.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = parts
                .iter()
                .map(|p| {
                    let item = p.items.first();
                    let action = item
                        .and_then(|i| i.get("action"))
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .to_string();
                    let title = item
                        .and_then(|i| i.get("title"))
                        .and_then(Value::as_str)
                        .map(clean_text)
                        .or_else(|| p.title.clone())
                        .unwrap_or_default();
                    let labels = item
                        .and_then(|i| i.get("labels"))
                        .and_then(|l| l.as_array())
                        .map(|a| crate::views::join_labels(a))
                        .unwrap_or_default();
                    vec![p.id.clone(), action, title, labels]
                })
                .collect();
            println!(
                "{}",
                format_table(&["PART", "ACTION", "TITLE", "LABELS"], rows)
            );
        }
    }
    Ok(())
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null => "—".to_string(),
        Value::String(s) => clean_text(s),
        Value::Array(a) => a.iter().map(value_display).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

pub fn changes_table(changes: &[ChangeView]) -> String {
    let rows: Vec<Vec<String>> = changes
        .iter()
        .map(|c| {
            vec![
                c.variable.clone(),
                value_display(&c.old_value),
                value_display(&c.new_value),
            ]
        })
        .collect();
    format_table(&["VARIABLE", "OLD", "NEW"], rows)
}

pub fn output_changes(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let changes: Vec<ChangeView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if changes.is_empty() {
                print_no_results("No changes.");
                return Ok(());
            }
            println!("{}", changes_table(&changes));
        }
    }
    Ok(())
}

pub fn output_status(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let s: StatusView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!("status: {}", clean_text(s.status.as_deref().unwrap_or("-")));
            for r in &s.responses {
                println!(
                    "  {}: {} {}",
                    r.user_id.as_deref().unwrap_or("-"),
                    clean_text(r.response.as_deref().unwrap_or("none")),
                    r.voted_at.as_deref().unwrap_or("")
                );
            }
            if let Some(a) = &s.autoapprove {
                println!("autoapprove: {}", clean_text(a));
            }
        }
    }
    Ok(())
}

pub fn output_children(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let children: Vec<ChildView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if children.is_empty() {
                print_no_results("No children.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = children
                .iter()
                .map(|c| {
                    vec![
                        c.id.clone(),
                        c.title.clone().map(|t| clean_text(&t)).unwrap_or_default(),
                        crate::views::join_labels(&c.labels),
                        c.link_id.clone().unwrap_or_default(),
                    ]
                })
                .collect();
            println!("{}", format_table(&["ID", "TITLE", "LABEL", "LINK"], rows));
        }
    }
    Ok(())
}

/// Render an optional metric value, dropping a trailing `.0` on integral floats.
fn fmt_num(v: Option<f64>) -> String {
    match v {
        None => String::new(),
        Some(n) if n.fract() == 0.0 => format!("{}", n as i64),
        Some(n) => format!("{n}"),
    }
}

pub fn link_table(links: &[LinkView]) -> String {
    let rows: Vec<Vec<String>> = links
        .iter()
        .map(|l| {
            vec![
                l.id.clone(),
                l.title.clone(),
                l.relation.clone().unwrap_or_default(),
                l.direction.clone().unwrap_or_default(),
                crate::views::join_labels(&l.labels),
            ]
        })
        .collect();
    format_table(&["ID", "TITLE", "RELATION", "DIRECTION", "LABELS"], rows)
}

pub fn output_links(
    data: &Value,
    meta: Option<&Value>,
    output: OutputFormat,
    supports_page: bool,
) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let links: Vec<LinkView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if links.is_empty() {
                print_no_results("No links.");
                return Ok(());
            }
            println!("{}", link_table(&links));
            if let Some(f) = pagination_footer(meta, supports_page) {
                println!("{f}");
            }
        }
    }
    Ok(())
}

pub fn insight_table(insights: &[InsightView]) -> String {
    let rows: Vec<Vec<String>> = insights
        .iter()
        .map(|i| {
            vec![
                i.type_.clone().unwrap_or_default(),
                i.title.clone().unwrap_or_default(),
                fmt_num(i.current_value),
                fmt_num(i.compare_value),
                i.goal.clone().unwrap_or_default(),
            ]
        })
        .collect();
    format_table(&["TYPE", "TITLE", "CURRENT", "COMPARE", "GOAL"], rows)
}

pub fn output_insights(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let insights: Vec<InsightView> =
                serde_json::from_value(data.clone()).unwrap_or_default();
            if insights.is_empty() {
                print_no_results("No insights.");
                return Ok(());
            }
            println!("{}", insight_table(&insights));
        }
    }
    Ok(())
}

pub fn insight_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let i: InsightView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!(
                "{}  [{}]",
                clean_text(&i.title.clone().unwrap_or_default()),
                clean_text(&i.type_.clone().unwrap_or_default())
            );
            if let Some(d) = i.description.as_deref().filter(|s| !s.is_empty()) {
                println!("description: {}", clean_text(d));
            }
            println!(
                "current: {}  (was {})",
                fmt_num(i.current_value),
                fmt_num(i.compare_value)
            );
            if let Some(g) = &i.goal {
                println!("goal: {}", clean_text(g));
            }
        }
    }
    Ok(())
}

/// Render an insight-history point array (`{date, value}`) as a DATE·VALUE table.
pub fn output_history(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let points = data.as_array().cloned().unwrap_or_default();
            if points.is_empty() {
                print_no_results("No history.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = points
                .iter()
                .map(|p| {
                    vec![
                        p.get("date")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        fmt_num(p.get("value").and_then(Value::as_f64)),
                    ]
                })
                .collect();
            println!("{}", format_table(&["DATE", "VALUE"], rows));
        }
    }
    Ok(())
}

pub fn webhook_table(hooks: &[WebhookView]) -> String {
    let rows: Vec<Vec<String>> = hooks
        .iter()
        .map(|w| {
            vec![
                w.id.clone(),
                w.url.clone().unwrap_or_default(),
                w.type_.clone().unwrap_or_default(),
                w.event.clone().unwrap_or_default(),
                w.label.clone().unwrap_or_default(),
            ]
        })
        .collect();
    format_table(&["ID", "URL", "TYPE", "EVENT", "LABEL"], rows)
}

pub fn output_webhooks(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let hooks: Vec<WebhookView> = serde_json::from_value(data.clone()).unwrap_or_default();
            if hooks.is_empty() {
                print_no_results("No webhooks.");
                return Ok(());
            }
            println!("{}", webhook_table(&hooks));
        }
    }
    Ok(())
}

pub fn webhook_detail(data: &Value, output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => print_json(data)?,
        OutputFormat::Text => {
            let w: WebhookView = serde_json::from_value(data.clone()).unwrap_or_default();
            println!(
                "{}  [{}]",
                clean_text(&w.url.clone().unwrap_or_default()),
                w.id
            );
            println!(
                "{} {}",
                clean_text(&w.type_.clone().unwrap_or_default()),
                clean_text(&w.event.clone().unwrap_or_default())
            );
            if let Some(l) = &w.label {
                println!("label: {}", clean_text(l));
            }
            if let Some(a) = &w.ancestor_id {
                println!("ancestor: {a}");
            }
            if let Some(c) = &w.created_at {
                println!("created: {c}");
            }
            if w.trigger_count.is_some() || w.error_count.is_some() {
                println!(
                    "triggers: {}  errors: {}",
                    w.trigger_count.unwrap_or(0),
                    w.error_count.unwrap_or(0)
                );
            }
        }
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
        let f = pagination_footer(Some(&meta), true).unwrap();
        assert!(f.contains("page 1/3"));
        assert!(f.contains("57 total"));
        assert!(f.contains("--page 2"));
        let suppressed = pagination_footer(Some(&meta), false).unwrap();
        assert!(suppressed.contains("page 1/3") && !suppressed.contains("for more"));
        let last = json!({"page":3,"total_pages":3,"total":57});
        assert!(!pagination_footer(Some(&last), true)
            .unwrap()
            .contains("for more"));
        assert!(pagination_footer(None, true).is_none());
    }

    #[test]
    fn skip_limit_footer_only_when_page_full() {
        // Full page (rows == limit) → footer pointing at the next skip.
        let f = skip_limit_footer(0, Some(50), 50).unwrap();
        assert!(f.contains("showing 1-50") && f.contains("--skip 50"));
        // With an offset.
        let f2 = skip_limit_footer(50, Some(50), 50).unwrap();
        assert!(f2.contains("showing 51-100") && f2.contains("--skip 100"));
        // Partial page → no footer.
        assert!(skip_limit_footer(0, Some(50), 30).is_none());
        // Default limit (50) applied when None; full page → footer.
        assert!(skip_limit_footer(0, None, 50).is_some());
        // Zero limit → no footer.
        assert!(skip_limit_footer(0, Some(0), 0).is_none());
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
    fn strip_html_removes_tags_and_collapses_whitespace() {
        assert_eq!(
            strip_html("<div><div>Hello   world</div></div>"),
            "Hello world"
        );
        assert_eq!(strip_html("plain"), "plain");
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

    #[test]
    fn tension_table_shows_status() {
        use crate::views::TensionView;
        let t: TensionView =
            serde_json::from_value(json!({"_id":"t1","title":"Gap","status":"draft","labels":[]}))
                .unwrap();
        let out = tension_table(&[t]);
        assert!(out.contains("Gap") && out.contains("draft") && out.contains("t1"));
    }

    #[test]
    fn changes_table_renders_old_arrow_new() {
        use crate::views::ChangeView;
        let c: ChangeView = serde_json::from_value(
            json!({"variable":"role.title","newValue":"New","oldValue":null}),
        )
        .unwrap();
        let out = changes_table(&[c]);
        assert!(out.contains("role.title") && out.contains("New"));
    }

    #[test]
    fn changes_table_joins_array_values_without_brackets() {
        use crate::views::ChangeView;
        let c: ChangeView = serde_json::from_value(
            json!({"variable":"circle.labels","newValue":["circle"],"oldValue":["role"]}),
        )
        .unwrap();
        let out = changes_table(&[c]);
        assert!(out.contains("circle") && out.contains("role") && !out.contains('['));
    }

    #[test]
    fn link_table_shows_relation_direction() {
        use crate::views::LinkView;
        let l: LinkView = serde_json::from_value(
            json!({"_id":"n1","title":"Mtg","relation":"meeting","direction":"outgoing","labels":[]}),
        )
        .unwrap();
        let out = link_table(&[l]);
        assert!(out.contains("Mtg") && out.contains("meeting") && out.contains("outgoing"));
    }

    #[test]
    fn fmt_num_drops_trailing_zero() {
        assert_eq!(fmt_num(Some(12.0)), "12");
        assert_eq!(fmt_num(Some(1.5)), "1.5");
        assert_eq!(fmt_num(None), "");
    }

    #[test]
    fn insight_table_formats_values() {
        use crate::views::InsightView;
        let i: InsightView = serde_json::from_value(
            json!({"type":"roles","title":"Roles","currentValue":12,"compareValue":10,"goal":"high"}),
        )
        .unwrap();
        let out = insight_table(&[i]);
        assert!(out.contains("roles") && out.contains("12") && out.contains("high"));
    }

    #[test]
    fn webhook_table_shows_url_type_event() {
        use crate::views::WebhookView;
        let w: WebhookView = serde_json::from_value(
            json!({"_id":"wh1","url":"https://x.test/h","type":"nest","event":"create"}),
        )
        .unwrap();
        let out = webhook_table(&[w]);
        assert!(
            out.contains("wh1")
                && out.contains("https://x.test/h")
                && out.contains("nest")
                && out.contains("create")
        );
    }

    #[test]
    fn sanitize_strips_control_but_keeps_tab_newline() {
        let dirty = "a\u{1b}]0;pwn\u{07}b\tc\nd\u{1b}[31mx";
        let clean = sanitize(dirty);
        assert!(!clean.contains('\u{1b}'), "ESC must be removed");
        assert!(!clean.contains('\u{07}'), "BEL must be removed");
        assert!(
            clean.contains('\t') && clean.contains('\n'),
            "tab/newline kept"
        );
    }

    #[test]
    fn clean_text_strips_escapes_and_html() {
        let v = clean_text("evil\u{1b}]0;pwn\u{07}<b>title</b>");
        assert!(!v.contains('\u{1b}') && !v.contains('\u{07}'));
        assert!(!v.contains('<') && !v.contains('>'));
        assert!(v.contains("title"));
    }

    #[test]
    fn clean_text_decodes_full_html_entities() {
        // The actual reported bug: BSVA titles showed `&nbsp;`.
        assert_eq!(clean_text("BSV&nbsp;Association"), "BSV Association");
        // Named entities beyond the basic five (html-escape couldn't do these).
        assert_eq!(clean_text("R&amp;D &mdash; done"), "R&D — done");
        assert_eq!(clean_text("it&#39;s &ldquo;quoted&rdquo;"), "it's “quoted”");
        // Tags stripped first, then entity decoded (so `&lt;` stays literal text).
        assert_eq!(clean_text("<b>a&amp;b</b>"), "a&b");
        assert_eq!(clean_text("3 &lt; 5"), "3 < 5");
    }

    #[test]
    fn clean_text_sanitizes_entity_encoded_control_chars() {
        // Decoding HTML entities can resurrect terminal control chars, so clean_text
        // re-sanitizes AFTER the decode. Cover every way an ESC/BEL/NUL can be smuggled
        // in as an entity — SEC-1 (terminal-escape injection) must stay closed.
        for payload in [
            "x&#27;[31mred",      // decimal ESC + CSI colour
            "x&#x1b;[31mred",     // hex ESC
            "bell&#7;here",       // decimal BEL
            "bell&#x07;here",     // hex BEL
            "nul&#0;here",        // NUL
            "t&#27;]0;pwned&#7;", // OSC window-title injection, both delimiters as entities
        ] {
            let v = clean_text(payload);
            assert!(
                !v.contains('\u{1b}') && !v.contains('\u{07}') && !v.contains('\u{0}'),
                "control char leaked for {payload:?}: {v:?}"
            );
        }
    }

    #[test]
    fn format_table_cleans_cell_escapes() {
        let out = format_table(&["T"], vec![vec!["x\u{1b}]0;pwn\u{07}y".to_string()]]);
        assert!(!out.contains('\u{1b}') && !out.contains('\u{07}'));
    }
}
