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

#[test]
fn nests_get_bare_hints_flag_still_precedes_the_id() {
    // `--hints` takes its level only as `--hints=summary`, so a bare `--hints`
    // before the id keeps parsing the way it did when it was a boolean.
    let out = run(&["nests", "get", "--hints", "n1"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("profile"),
        "expected to get past parsing, got: {stderr}"
    );
    let bad = run(&["nests", "get", "n1", "--hints=everything"]);
    assert!(String::from_utf8_lossy(&bad.stderr).contains("possible values"));
}

#[test]
fn read_only_env_accepts_one_and_zero() {
    // The read-only-key error tells people to set NESTR_READ_ONLY=1, so it must parse.
    for value in ["1", "0", "true", "false"] {
        let tmp = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_nestr"))
            .args(["auth", "status"])
            .env("NESTR_HOME", tmp.path())
            .env("NESTR_READ_ONLY", value)
            .env_remove("NESTR_API_KEY")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("invalid value"),
            "NESTR_READ_ONLY={value} was rejected: {stderr}"
        );
    }
}

#[test]
fn nests_update_clear_due_conflicts_with_due() {
    let out = run(&[
        "nests",
        "update",
        "n1",
        "--due",
        "2026-07-01",
        "--clear-due",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("cannot be used with"),
        "expected a clap conflict error, got: {stderr}"
    );
}

#[test]
fn nests_update_clear_proxy_assign_conflicts_with_proxy_assign() {
    let out = run(&[
        "nests",
        "update",
        "n1",
        "--proxy-assign",
        "role-1",
        "--clear-proxy-assign",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("cannot be used with"),
        "expected a clap conflict error, got: {stderr}"
    );
}

#[test]
fn nests_activity_depth_is_a_number_or_all() {
    let out = run(&["nests", "activity", "n1", "--depth", "deep"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("invalid value"),
        "expected a clap value error, got: {stderr}"
    );
}

#[test]
fn nests_activity_interval_needs_follow() {
    let out = run(&["nests", "activity", "n1", "--interval", "30"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("--follow"),
        "expected a clap requires error, got: {stderr}"
    );
}

#[test]
fn nests_activity_limit_is_capped_at_200() {
    let out = run(&["nests", "activity", "n1", "--limit", "201"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains("invalid value"),
        "expected a clap range error, got: {stderr}"
    );
}
