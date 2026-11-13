use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::insights;
use nestr_cli::error::NestrError;
use nestr_cli::render;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn period_meta() -> serde_json::Value {
    json!({
        "period": {"from": "2026-10-01T00:00:00.000Z", "to": "2026-11-01T00:00:00.000Z"},
        "comparePeriod": {"from": "2026-09-01T00:00:00.000Z", "to": "2026-10-01T00:00:00.000Z"},
        "valueSemantics": {
            "point_in_time": "currentValue is the value at period.to. compareValue is the last value recorded before period.from.",
            "cumulative": "currentValue is the total within period. compareValue is the total within comparePeriod."
        },
        "nestId": "ws1",
        "userId": null
    })
}

#[tokio::test]
async fn list_hits_insights_path_and_keeps_the_period() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta": period_meta(),
            "data":[{"type":"roles","title":"Roles","currentValue":12}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = insights::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["type"], "roles");
    assert_eq!(meta.unwrap()["period"]["from"], "2026-10-01T00:00:00.000Z");
}

#[tokio::test]
async fn list_passes_period_and_circle() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights"))
        .and(query_param("from", "2026-10-01"))
        .and(query_param("to", "2026-11-01"))
        .and(query_param("nestId", "c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta": period_meta(),"data":[]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let params = insights::list_params(&insights::Scope {
        user: None,
        nest: Some("c1".into()),
        from: Some("2026-10-01".into()),
        to: Some("2026-11-01".into()),
        include_sub_circles: None,
    });
    let pairs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (data, _) = insights::fetch_list(&client, "ws1", &pairs).await.unwrap();
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_hits_metric_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/roles"))
        .and(query_param("to", "2026-11-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta": period_meta(),
            "data":{"type":"roles","title":"Roles","currentValue":12}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = insights::fetch_get(&client, "ws1", "roles", &[("to", "2026-11-01")])
        .await
        .unwrap();
    assert_eq!(data["currentValue"], 12);
    assert!(meta.is_some());
}

#[tokio::test]
async fn history_passes_from_param() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/roles/history"))
        .and(query_param("from", "2026-01-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":[{"date":"2026-01-01","value":11}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = insights::fetch_history(&client, "ws1", "roles", &[("from", "2026-01-01")])
        .await
        .unwrap();
    assert_eq!(data[0]["value"], 11);
}

#[tokio::test]
async fn history_scopes_to_a_circle_or_a_person() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/comments/history"))
        .and(query_param("userId", "me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta":{"nestId":"ws1","userId":"u1"},"data":[]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/comments/history"))
        .and(query_param("nestId", "c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta":{"nestId":"c1","userId":null},"data":[]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    insights::fetch_history(&client, "ws1", "comments", &[("userId", "me")])
        .await
        .unwrap();
    insights::fetch_history(&client, "ws1", "comments", &[("nestId", "c1")])
        .await
        .unwrap();
}

#[tokio::test]
async fn list_passes_user_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights"))
        .and(query_param("userId", "me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":[]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = insights::fetch_list(&client, "ws1", &[("userId", "me")])
        .await
        .unwrap();
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn below_pro_the_server_says_what_pro_adds() {
    let server = MockServer::start().await;
    let message = "Insights for one circle is part of the Pro plan. Pro adds insights for one circle, \
                   personal insights and comparing circles side by side. Workspace-level insights stay \
                   available on every plan. A workspace admin can upgrade in the workspace settings under Billing.";
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights"))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({
            "status": "error",
            "message": message,
            "upgrade": {"requiredPlan": "plan_pro", "requested": "circle", "proAdds": []}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = insights::fetch_list(&client, "ws1", &[("nestId", "c1")])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::PlanRequired(_)), "{err:?}");
    assert!(err.to_string().contains(message), "{err}");
}

fn compare_body() -> serde_json::Value {
    json!({
        "status": "success",
        "meta": {
            "period": {"from": "2026-10-01T00:00:00.000Z", "to": "2026-11-01T23:59:59.999Z"},
            "comparePeriod": {"from": "2026-08-31T00:00:00.000Z", "to": "2026-10-01T00:00:00.000Z"},
            "valueSemantics": {},
            "metricId": "tactical_completed", "parentNestId": "ws1",
            "valueType": "percentage", "goal": "high"
        },
        "data": [
            {"nestId": "ws1", "title": "Acme", "isParent": true, "value": 30, "compareValue": 20,
             "delta": 10, "deltaPercent": 50, "direction": "up", "excludeFromComparison": true},
            {"nestId": "c1", "title": "Sales", "isParent": false, "value": 4, "compareValue": 0,
             "delta": 4, "deltaPercent": "Infinity", "direction": "up", "excludeFromComparison": false},
            {"nestId": "c2", "title": "Ops", "isParent": false, "value": 6, "compareValue": 8,
             "delta": -2, "deltaPercent": -25, "direction": "down", "excludeFromComparison": false}
        ]
    })
}

#[tokio::test]
async fn compare_hits_the_circles_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/tactical_completed/circles"))
        .and(query_param("parentNestId", "c9"))
        .and(query_param("includeSubCircles", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(compare_body()))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = insights::fetch_compare(
        &client,
        "ws1",
        "tactical_completed",
        &[("parentNestId", "c9"), ("includeSubCircles", "false")],
    )
    .await
    .unwrap();
    assert_eq!(data.as_array().unwrap().len(), 3);
    assert_eq!(meta.unwrap()["goal"], "high");
}

#[test]
fn insights_text_prints_both_periods_and_how_each_kind_reads() {
    let data = json!([
        {"type":"roles","title":"Roles","currentValue":12,"compareValue":10,
         "dataType":"point_in_time","goal":"high"}
    ]);
    let out = render::insights_text(&data, Some(&period_meta()));
    assert!(
        out.contains("2026-10-01T00:00:00.000Z → 2026-11-01T00:00:00.000Z"),
        "{out}"
    );
    assert!(
        out.contains("2026-09-01T00:00:00.000Z → 2026-10-01T00:00:00.000Z"),
        "{out}"
    );
    assert!(out.contains("point_in_time"), "{out}");
    assert!(
        out.contains("the last value recorded before period.from"),
        "{out}"
    );
    // Only the kinds present get their reading.
    assert!(!out.contains("the total within comparePeriod"), "{out}");
}

#[test]
fn insights_text_without_meta_is_just_the_table() {
    let data = json!([{"type":"roles","title":"Roles","currentValue":12}]);
    let out = render::insights_text(&data, None);
    assert!(out.contains("roles") && out.contains("12"), "{out}");
    assert!(!out.contains("period"), "{out}");
}

#[test]
fn insight_detail_text_reads_the_value_against_its_period() {
    let data = json!({"type":"comments","title":"Comments","currentValue":40,"compareValue":25,
                      "dataType":"cumulative","goal":"high"});
    let out = render::insight_detail_text(&data, Some(&period_meta()));
    assert!(out.contains("Comments  [comments]"), "{out}");
    assert!(out.contains("40"), "{out}");
    assert!(out.contains("25"), "{out}");
    assert!(
        out.contains("2026-10-01T00:00:00.000Z → 2026-11-01T00:00:00.000Z"),
        "{out}"
    );
    assert!(out.contains("the total within comparePeriod"), "{out}");
}

#[test]
fn compare_text_keeps_every_row_and_sets_the_parent_apart() {
    let body = compare_body();
    let out = render::compare_text(&body["data"], body.get("meta"));
    assert!(out.contains("Sales") && out.contains("Ops"), "{out}");
    assert!(out.contains("Infinity"), "{out}");
    assert!(out.contains("-25%"), "{out}");
    let parent = out.lines().find(|l| l.contains("Acme")).unwrap();
    assert!(parent.contains("parent"), "{parent}");
    assert!(out.contains("goal: high"), "{out}");
    assert!(out.contains("format: percentage"), "{out}");
    assert!(
        out.contains("2026-08-31T00:00:00.000Z → 2026-10-01T00:00:00.000Z"),
        "{out}"
    );
}

// ---- 0.5.2: never present numbers the server didn't scope the way they were asked for.
// Replies shaped like `origin/master` (production: no `meta`, no from/to) and like
// `origin/develop` (always `meta.period`; history carries `meta.nestId`/`meta.userId`).

use insights::{confirm, parse_day, Asked, Route};

fn today() -> i64 {
    parse_day("2026-11-11").unwrap()
}

fn asked<'a>(
    from: Option<&'a str>,
    to: Option<&'a str>,
    nest: Option<&'a str>,
    user: Option<&'a str>,
) -> Asked<'a> {
    Asked {
        from,
        to,
        nest,
        user,
    }
}

#[test]
fn parse_day_reads_dates_and_timestamps() {
    assert_eq!(parse_day("1970-01-02"), Some(1));
    assert_eq!(parse_day("2026-11-11T13:00:00Z"), parse_day("2026-11-11"));
    assert_eq!(
        parse_day("2024-03-01").unwrap() - parse_day("2024-02-28").unwrap(),
        2
    );
    assert_eq!(parse_day("next week"), None);
    assert_eq!(parse_day("2026-13-01"), None);
}

#[test]
fn a_develop_reply_confirms_every_option() {
    let m = period_meta();
    let all = asked(Some("2024-01-01"), Some("2024-02-01"), None, None);
    for route in [Route::List, Route::Get, Route::Compare] {
        assert!(matches!(confirm(route, &all, Some(&m), today()), Ok(None)));
    }
    // An old --to is fine where the server states the period.
    let old_to = asked(None, Some("2025-01-01"), None, None);
    assert!(matches!(
        confirm(Route::List, &old_to, Some(&m), today()),
        Ok(None)
    ));
    let user = asked(None, None, None, Some("u1"));
    let hist_user = json!({"nestId": "ws1", "userId": "u1"});
    assert!(matches!(
        confirm(Route::History, &user, Some(&hist_user), today()),
        Ok(None)
    ));
    let nest = asked(Some("2026-01-01"), Some("2026-02-01"), Some("c1"), None);
    let hist_nest = json!({"nestId": "c1", "userId": null});
    assert!(matches!(
        confirm(Route::History, &nest, Some(&hist_nest), today()),
        Ok(None)
    ));
}

#[test]
fn a_master_reply_refuses_from_on_list() {
    let err = confirm(
        Route::List,
        &asked(Some("2024-01-01"), Some("2024-02-01"), None, None),
        None,
        today(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, NestrError::Ignored(_)), "{err:?}");
    assert!(msg.contains("ignores --from"), "{msg}");
    assert!(msg.contains("needs a newer Nestr server"), "{msg}");
}

#[test]
fn a_master_reply_takes_a_recent_to_and_says_what_period_it_covers() {
    let note = confirm(
        Route::List,
        &asked(None, Some("2026-11-01"), None, None),
        None,
        today(),
    )
    .unwrap()
    .expect("a note on the period");
    assert!(note.contains("one month before today"), "{note}");
}

#[test]
fn a_master_reply_refuses_a_to_from_before_its_period_starts() {
    for to in ["2025-01-01", "2026-10-14"] {
        let err = confirm(
            Route::List,
            &asked(None, Some(to), None, None),
            None,
            today(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("--to"), "{err}");
    }
    // A date it can't place is refused too, rather than guessed at.
    assert!(confirm(
        Route::List,
        &asked(None, Some("last tuesday"), None, None),
        None,
        today()
    )
    .is_err());
}

#[test]
fn a_master_reply_needs_nothing_confirmed_for_scope_or_no_options() {
    let none = asked(None, None, None, None);
    for route in [Route::List, Route::Get, Route::History, Route::Compare] {
        assert!(matches!(confirm(route, &none, None, today()), Ok(None)));
    }
    // Master's list honours userId and nestId.
    let scoped = asked(None, None, Some("c1"), None);
    assert!(matches!(
        confirm(Route::List, &scoped, None, today()),
        Ok(None)
    ));
    // Both servers honour history's from/to.
    let span = asked(Some("2026-01-01"), Some("2026-02-01"), None, None);
    assert!(matches!(
        confirm(Route::History, &span, None, today()),
        Ok(None)
    ));
}

#[test]
fn a_master_reply_refuses_get_with_a_period() {
    for a in [
        asked(Some("2026-10-01"), None, None, None),
        asked(None, Some("2026-11-01"), None, None),
    ] {
        let err = confirm(Route::Get, &a, None, today()).unwrap_err();
        assert!(err.to_string().contains("insights get"), "{err}");
    }
}

#[test]
fn a_master_reply_refuses_history_for_a_circle_or_a_person() {
    let err = confirm(
        Route::History,
        &asked(None, None, Some("c1"), None),
        None,
        today(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("ignores --nest"), "{err}");
    let err = confirm(
        Route::History,
        &asked(None, None, None, Some("u1")),
        None,
        today(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("ignores --user"), "{err}");
    // A reply scoped to someone else is no confirmation either.
    let other = json!({"nestId": "ws1", "userId": "u2"});
    assert!(confirm(
        Route::History,
        &asked(None, None, None, Some("u1")),
        Some(&other),
        today()
    )
    .is_err());
}

#[test]
fn list_params_send_end_date_alongside_to() {
    let params = insights::list_params(&insights::Scope {
        to: Some("2026-11-01".into()),
        ..Default::default()
    });
    assert!(params.contains(&("to", "2026-11-01".to_string())));
    assert!(params.contains(&("endDate", "2026-11-01".to_string())));
    let none = insights::list_params(&insights::Scope::default());
    assert!(!none.iter().any(|(k, _)| *k == "endDate"));
}

// ---- The real binary against master- and develop-shaped servers.

fn run_cli(host: &str, args: &[&str]) -> std::process::Output {
    use nestr_cli::config::{save_profile, AuthKind, CredentialStorage, Profile};
    // NESTR_HOME is process-wide, and these tests run in parallel.
    static HOME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = HOME.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    let profile = Profile {
        auth: AuthKind::ApiKey,
        credential_storage: CredentialStorage::File,
        host: host.into(),
        workspace_id: "ws1".into(),
        api_key: Some("k".into()),
        oauth_client_id: None,
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: None,
        client_instance_id: None,
        default_output_format: None,
    };
    save_profile("t", &profile).unwrap();
    std::env::remove_var("NESTR_HOME");
    drop(guard);
    std::process::Command::new(env!("CARGO_BIN_EXE_nestr"))
        .args(["--profile", "t"])
        .args(args)
        .env("NESTR_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .env_remove("NESTR_API_KEY")
        .env_remove("NESTR_PROFILE")
        .output()
        .unwrap()
}

fn master_list() -> serde_json::Value {
    json!({"status":"success","data":[{"type":"roles","title":"Roles","currentValue":12}]})
}

fn develop_list() -> serde_json::Value {
    json!({"status":"success","meta": period_meta(),
           "data":[{"type":"roles","title":"Roles","currentValue":12}]})
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_refuses_from_on_a_master_server_in_text_and_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/workspaces/ws1/insights"))
        .respond_with(ResponseTemplate::new(200).set_body_json(master_list()))
        .mount(&server)
        .await;
    for out_fmt in ["text", "json"] {
        let out = run_cli(
            &server.uri(),
            &["-o", out_fmt, "insights", "list", "--from", "2024-01-01"],
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{out_fmt}: {stderr}");
        assert!(out.stdout.is_empty(), "{out_fmt}: printed numbers");
        assert!(stderr.contains("ignores --from"), "{stderr}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_prints_a_develop_server_reply_for_the_same_options() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/workspaces/ws1/insights"))
        .and(query_param("from", "2024-01-01"))
        .and(query_param("to", "2025-01-01"))
        .and(query_param("endDate", "2025-01-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(develop_list()))
        .expect(1)
        .mount(&server)
        .await;
    let out = run_cli(
        &server.uri(),
        &[
            "-o",
            "text",
            "insights",
            "list",
            "--from",
            "2024-01-01",
            "--end-date",
            "2025-01-01",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("Roles"));
    assert!(stderr.is_empty(), "no warning on a new server: {stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_refuses_an_old_to_on_a_master_server() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/workspaces/ws1/insights"))
        .and(query_param("endDate", "2025-01-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(master_list()))
        .expect(1)
        .mount(&server)
        .await;
    let out = run_cli(&server.uri(), &["insights", "list", "--to", "2025-01-01"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_resolves_user_me_to_the_callers_own_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"u42","profile":{"fullName":"A"}}
        })))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/workspaces/ws1/insights"))
        .and(query_param("userId", "u42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(master_list()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/workspaces/ws1/insights/comments/history"))
        .and(query_param("userId", "u42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta":{"nestId":"ws1","userId":"u42"},"data":[]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let out = run_cli(&server.uri(), &["insights", "list", "--user", "me"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run_cli(
        &server.uri(),
        &["insights", "history", "comments", "--user", "me"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_compare_on_a_server_without_it_says_so_with_a_hint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/workspaces/ws1/insights/tactical_completed/circles",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<!DOCTYPE html><html><body>app</body></html>",
            "text/html; charset=utf-8",
        ))
        .mount(&server)
        .await;
    let out = run_cli(
        &server.uri(),
        &["insights", "compare", "tactical_completed"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert!(stderr.contains("needs a newer Nestr server"), "{stderr}");
    assert!(stderr.contains("insights list --nest"), "{stderr}");
    assert!(!stderr.contains("<html"), "{stderr}");
}
