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
    let data = insights::fetch_history(&client, "ws1", "roles", &[("from", "2026-01-01")])
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
