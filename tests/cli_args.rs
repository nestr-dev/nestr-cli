//! COR-4 / COR-9 / COR-14: clap must reject these invocations at parse time,
//! before any profile/config is consulted. NESTR_HOME points at an empty temp
//! dir so a parse *success* would fail later on "profile not found" instead.
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    let tmp = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_nestr"))
        .args(args)
        .env("NESTR_HOME", tmp.path())
        .env_remove("NESTR_API_KEY")
        .output()
        .unwrap()
}

#[test]
fn groups_set_requires_at_least_one_name() {
    let out = run(&["users", "groups", "set", "u1"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("required") || stderr.contains("<NAMES>"),
        "expected a clap arity error, got: {stderr}"
    );
}

#[test]
fn search_page_conflicts_with_in() {
    let out = run(&["search", "q", "--in", "n9", "--page", "2"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflicts"),
        "expected a clap conflict error, got: {stderr}"
    );
}

#[test]
fn notifications_type_rejects_unknown_value() {
    let out = run(&["notifications", "list", "--type", "unread"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("invalid value") || stderr.contains("possible values"),
        "expected a clap value error, got: {stderr}"
    );
}

#[test]
fn agents_mode_set_autonomous_requires_recheck() {
    let out = run(&[
        "agents",
        "mode",
        "set",
        "a1",
        "--nest",
        "n1",
        "--mode",
        "autonomous",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("--recheck"),
        "expected a missing --recheck error, got: {stderr}"
    );
}

#[test]
fn connectors_register_template_conflicts_with_type() {
    let out = run(&[
        "connectors",
        "register",
        "--template",
        "xero",
        "--type",
        "mcp",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("cannot be used with"),
        "expected a clap conflict error, got: {stderr}"
    );
}

#[test]
fn connectors_bind_rejects_unknown_owner_type() {
    let out = run(&[
        "connectors",
        "bind",
        "k1",
        "--owner-type",
        "team",
        "--owner",
        "x",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("invalid value") || stderr.contains("possible values"),
        "expected a clap value error, got: {stderr}"
    );
}

#[test]
fn dm_update_requires_a_change() {
    let out = run(&["dm", "update", "t1"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("required"),
        "expected a clap missing-argument error, got: {stderr}"
    );
}

#[test]
fn unread_conflicts_with_read() {
    for args in [
        &["dm", "posts", "t1", "--unread", "--read"][..],
        &["queues", "threads", "support", "--unread", "--read"][..],
        &["comments", "list", "n1", "--unread", "--read"][..],
    ] {
        let out = run(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success());
        assert!(
            stderr.contains("cannot be used with"),
            "expected a clap conflict for {args:?}, got: {stderr}"
        );
    }
}
