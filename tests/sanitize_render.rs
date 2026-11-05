//! SEC-1 regression: attacker-authored nest titles must not emit raw terminal
//! control bytes in default (text) table output.
use nestr_cli::render::nest_table;
use nestr_cli::views::CompactNest;

#[test]
fn hostile_title_renders_without_control_bytes() {
    let n: CompactNest = serde_json::from_value(serde_json::json!({
        "_id": "abc",
        "title": "evil\u{1b}]0;pwn\u{07}\u{1b}]52;c;ZXZpbA==\u{07}title",
        "labels": []
    }))
    .unwrap();
    let out = nest_table(&[n]);
    assert!(!out.contains('\u{1b}'), "ESC leaked into table output");
    assert!(!out.contains('\u{07}'), "BEL leaked into table output");
    assert!(out.contains("title"), "legitimate text preserved");
}

#[test]
fn colour_codes_in_a_title_leave_nothing_behind() {
    let n: CompactNest = serde_json::from_value(serde_json::json!({
        "_id": "abc",
        "title": "Review \u{1b}[1;31mpack\u{1b}[0m\u{1b}[31m",
        "labels": []
    }))
    .unwrap();
    let out = nest_table(&[n]);
    assert!(out.contains("Review pack"), "title text kept: {out}");
    assert!(
        !out.contains("[31m") && !out.contains("[0m"),
        "colour code leaked: {out}"
    );
}
