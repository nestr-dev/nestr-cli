use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::series;
use nestr_cli::render;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn series_list() -> serde_json::Value {
    json!({"status": "success", "data": [
        {"fieldId": "f1", "fieldCode": "metric-data", "seriesId": null, "key": "metric-data",
         "dataType": "point_in_time", "hidden": false, "trackedField": "metric-value",
         "current": {"id": "p9", "date": "2026-11-02T09:00:00.000Z", "value": 42},
         "endpoints": []},
        {"fieldId": "f2", "fieldCode": "metric-data", "seriesId": null, "key": "f2",
         "dataType": "cumulative", "hidden": true, "trackedField": null,
         "current": null, "endpoints": []}
    ]})
}

#[tokio::test]
async fn list_unwraps_the_series() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/series"))
        .respond_with(ResponseTemplate::new(200).set_body_json(series_list()))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = series::fetch_series(&client, "n1").await.unwrap();
    assert_eq!(data[0]["key"], "metric-data");
    assert_eq!(data[1]["hidden"], true);
}

#[test]
fn series_text_marks_a_hidden_series_read_only() {
    let data = series_list()["data"].clone();
    let out = render::series_text(&data);
    assert!(out.contains("metric-data"), "{out}");
    assert!(out.contains("42 at 2026-11-02T09:00:00.000Z"), "{out}");
    assert!(out.contains("metric-value"), "{out}");
    let hidden_row = out.lines().find(|l| l.contains("f2")).unwrap();
    assert!(hidden_row.contains("hidden"), "{hidden_row}");
}

#[tokio::test]
async fn points_reads_one_series_with_its_meta() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/series/metric-data/points"))
        .and(query_param("from", "2026-01-01"))
        .and(query_param("order", "desc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "meta": {"count": 1, "limit": 1, "order": "desc", "has_more": true},
            "data": [{"id": "p1", "date": "2026-01-05T09:00:00.000Z", "value": 12}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = series::fetch_points(
        &client,
        "n1",
        "metric-data",
        None,
        &[("from", "2026-01-01"), ("order", "desc")],
    )
    .await
    .unwrap();
    assert_eq!(data[0]["id"], "p1");
    assert_eq!(meta.unwrap()["has_more"], true);
}

#[tokio::test]
async fn points_addresses_a_multi_series_field_by_series_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/series/burndown/s2/points"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success", "meta": {"has_more": false}, "data": []
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = series::fetch_points(&client, "n1", "burndown", Some("s2"), &[])
        .await
        .unwrap();
    assert!(data.as_array().unwrap().is_empty());
}

#[test]
fn points_text_says_to_narrow_the_range_when_more_remain() {
    let data = json!([{"id": "p1", "date": "2026-01-05T09:00:00.000Z", "value": 12.5}]);
    let out = render::points_text(&data, Some(&json!({"has_more": true})));
    assert!(out.contains("p1"), "{out}");
    assert!(out.contains("12.5"), "{out}");
    assert!(out.contains("--from/--to"), "{out}");
    let done = render::points_text(&data, Some(&json!({"has_more": false})));
    assert!(!done.contains("--from/--to"), "{done}");
}

#[test]
fn parses_date_equals_value_points() {
    assert_eq!(
        series::parse_point("2026-01-05=12").unwrap(),
        json!({"date": "2026-01-05", "value": 12.0})
    );
    assert_eq!(
        series::parse_point("2026-01-05T09:00:00Z=-3.5").unwrap(),
        json!({"date": "2026-01-05T09:00:00Z", "value": -3.5})
    );
    assert!(series::parse_point("2026-01-05").is_err());
    assert!(series::parse_point("2026-01-05=abc").is_err());
    assert!(series::parse_point("2026-01-05=NaN").is_err());
    assert!(series::parse_point("=4").is_err());
}

#[tokio::test]
async fn add_posts_an_array_and_counts_the_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/series/metric-data/points"))
        .and(body_json(json!([
            {"date": "2026-01-05", "value": 12.0},
            {"date": "bad", "value": 3.0}
        ])))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "meta": {"created": 1, "failed": 1},
            "data": [
                {"index": 0, "status": "created",
                 "data": {"id": "p1", "date": "2026-01-05T00:00:00.000Z", "value": 12}},
                {"index": 1, "status": "error",
                 "message": "date must be an ISO date or epoch milliseconds"}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let points = vec![
        json!({"date": "2026-01-05", "value": 12.0}),
        json!({"date": "bad", "value": 3.0}),
    ];
    let outcome = series::add_points(&client, "n1", "metric-data", None, &points)
        .await
        .unwrap();
    assert_eq!(outcome.created, 1);
    assert_eq!(outcome.failed, 1);
    assert_eq!(outcome.results[1]["status"], "error");
}

#[tokio::test]
async fn add_splits_a_backfill_into_requests_of_500() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/series/metric-data/points"))
        .respond_with(|req: &wiremock::Request| {
            let items: Vec<serde_json::Value> = serde_json::from_slice(&req.body).unwrap();
            assert!(
                items.len() <= 500,
                "sent {} points in one request",
                items.len()
            );
            let data: Vec<_> = (0..items.len())
                .map(|i| json!({"index": i, "status": "created", "data": {"id": format!("p{i}")}}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "status": "success",
                "meta": {"created": items.len(), "failed": 0},
                "data": data
            }))
        })
        .expect(2)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let points: Vec<_> = (0..501)
        .map(|i| json!({"date": i.to_string(), "value": 1.0}))
        .collect();
    let outcome = series::add_points(&client, "n1", "metric-data", None, &points)
        .await
        .unwrap();
    assert_eq!(outcome.created, 501);
    assert_eq!(outcome.results.len(), 501);
    // Indexes run across the whole backfill, not per request.
    assert_eq!(outcome.results[500]["index"], 500);
}

#[tokio::test]
async fn a_backfill_that_fails_partway_says_where_to_resend_from() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/series/metric-data/points"))
        .respond_with(|req: &wiremock::Request| {
            let items: Vec<serde_json::Value> = serde_json::from_slice(&req.body).unwrap();
            if items.len() < 500 {
                return ResponseTemplate::new(500)
                    .set_body_json(json!({"status": "error", "message": "boom"}));
            }
            let data: Vec<_> = (0..items.len())
                .map(|i| json!({"index": i, "status": "created", "data": {"id": format!("p{i}")}}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "status": "success", "meta": {"created": items.len(), "failed": 0}, "data": data
            }))
        })
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let points: Vec<_> = (0..501)
        .map(|i| json!({"date": i.to_string(), "value": 1.0}))
        .collect();
    let outcome = series::add_points(&client, "n1", "metric-data", None, &points)
        .await
        .unwrap();
    assert_eq!(outcome.created, 500);
    let why = outcome.failure(points.len()).unwrap();
    assert!(why.contains("boom"), "{why}");
    assert!(why.contains("from point #501"), "{why}");
    assert!(!why.contains("refused points listed"), "{why}");

    let with_refusals = series::AddOutcome {
        created: 499,
        failed: 1,
        results: vec![],
        stopped: Some((500, "boom".into())),
    };
    let why = with_refusals.failure(501).unwrap();
    assert!(
        why.contains("from point #501 on, plus the 1 refused points listed above"),
        "{why}"
    );
}

#[test]
fn a_fully_refused_backfill_says_nothing_was_added() {
    let outcome = series::AddOutcome {
        created: 0,
        failed: 2,
        results: vec![],
        stopped: None,
    };
    assert_eq!(
        outcome.failure(2).unwrap(),
        "all 2 points were refused; nothing was added"
    );
    let partly = series::AddOutcome {
        created: 1,
        failed: 1,
        ..Default::default()
    };
    assert_eq!(
        partly.failure(2).unwrap(),
        "1 of 2 points were refused; 1 was added"
    );
    let ok = series::AddOutcome {
        created: 2,
        ..Default::default()
    };
    assert!(ok.failure(2).is_none());
}

#[test]
fn add_text_lists_each_refused_point() {
    let outcome = series::AddOutcome {
        created: 1,
        failed: 1,
        stopped: None,
        results: vec![
            json!({"index": 0, "status": "created", "data": {"id": "p1"}}),
            json!({"index": 1, "status": "error", "message": "date must be an ISO date"}),
        ],
    };
    let points = vec![
        json!({"date": "2026-01-05", "value": 12.0}),
        json!({"date": "bad", "value": 3.0}),
    ];
    let out = series::add_text(&outcome, &points, "metric-data");
    assert!(out.contains("Added 1 point"), "{out}");
    assert!(out.contains("bad=3:"), "{out}");
    assert!(out.contains("date must be an ISO date"), "{out}");
}

#[tokio::test]
async fn edit_patches_only_what_changes() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1/series/metric-data/points/p1"))
        .and(body_json(json!({"value": -2.0})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "p1", "date": "2026-01-05T00:00:00.000Z", "value": -2}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = series::edit_body(None, Some(-2.0)).unwrap();
    let data = series::update_point(&client, "n1", "metric-data", None, "p1", &body)
        .await
        .unwrap();
    assert_eq!(data["value"], -2);
}

#[test]
fn edit_needs_a_date_or_a_value() {
    assert!(series::edit_body(None, None).is_err());
    assert!(series::edit_body(None, Some(f64::INFINITY)).is_err());
    assert_eq!(
        series::edit_body(Some("2026-02-01".into()), None).unwrap(),
        json!({"date": "2026-02-01"})
    );
}

#[tokio::test]
async fn delete_removes_one_point() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/n1/series/burndown/s2/points/p1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success", "data": {"id": "p1"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = series::delete_point(&client, "n1", "burndown", Some("s2"), "p1")
        .await
        .unwrap();
    assert_eq!(data["id"], "p1");
}

#[tokio::test]
async fn a_write_to_a_hidden_series_surfaces_the_server_reason() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/series/f2/points"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "status": "error",
            "message": "The series \"f2\" is not in use on this nest, so it cannot be written."
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = series::add_points(
        &client,
        "n1",
        "f2",
        None,
        &[json!({"date": "2026-01-05", "value": 1.0})],
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("not in use on this nest"), "{err}");
}
