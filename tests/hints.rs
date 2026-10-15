use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::{hints, nests, search};
use nestr_cli::render;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn rollup_passes_types_severity_and_sample_size() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/c1/hints"))
        .and(query_param(
            "hintTypes",
            "unassigned_role,project_waiting_no_reason",
        ))
        .and(query_param("minSeverity", "warning"))
        .and(query_param("sampleSize", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {
                "scope": {"_id": "c1", "workspaceId": "ws1", "isWorkspaceRoot": false},
                "results": [{"type": "unassigned_role", "severity": "warning", "count": 3,
                             "counts": "nests", "sample": [], "sampleTruncated": true}],
                "notComputed": {"no_active_work": "Needs a per-role child count."}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let types = vec![
        "unassigned_role".to_string(),
        "project_waiting_no_reason".to_string(),
    ];
    let data = hints::fetch_rollup(&client, "c1", &types, Some("warning"), Some(0))
        .await
        .unwrap();
    assert_eq!(data["results"][0]["count"], 3);
    assert!(data["notComputed"]["no_active_work"].is_string());
}

#[test]
fn rollup_rows_show_a_sample_and_how_many_more() {
    let result = json!({
        "type": "unassigned_role", "severity": "warning", "count": 5,
        "sample": [{"_id": "r1", "title": "Scribe"}, {"_id": "r2", "title": "Facilitator"}],
        "sampleTruncated": true
    });
    assert_eq!(hints::sample_cell(&result), "Scribe, Facilitator (+3 more)");
}

#[tokio::test]
async fn strict_search_appends_the_operator_once() {
    assert_eq!(
        search::strict_query("label:role", true),
        "label:role strict:true"
    );
    assert_eq!(
        search::strict_query("label:role strict:true", true),
        "label:role strict:true"
    );
    assert_eq!(search::strict_query("label:role", false), "label:role");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/search"))
        .and(query_param("search", "lable:role strict:true"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "status": "error",
            "message": "Search rejected in strict mode: Unknown operator lable:."
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let q = search::strict_query("lable:role", true);
    let err = search::run_search(&client, "ws1", &q, None, &[])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("strict mode"), "{err}");
}

#[tokio::test]
async fn nest_get_sends_the_hint_level_and_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .and(query_param("hints", "summary"))
        .and(query_param("hintTypes", "stale_work"))
        .and(query_param("minSeverity", "suggestion"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"_id": "n1", "title": "x", "hints": [{"type": "stale_work", "label": "stale"}]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let opts = nests::HintOpts {
        level: Some("summary".into()),
        types: vec!["stale_work".into()],
        min_severity: Some("suggestion".into()),
    };
    let owned = opts.params();
    let params: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let data = nests::fetch_get(&client, "n1", &params).await.unwrap();
    assert_eq!(data["_id"], "n1");
    // A nest's own hints are rendered with the nest; only envelope-level hints are
    // notices, so they don't print twice.
    assert!(client.notices().is_empty());
}

#[test]
fn hint_lines_name_severity_label_and_the_follow_up_search() {
    let nest = json!({
        "_id": "c1", "title": "Ops",
        "hints": [
            {"type": "unassigned_role", "severity": "warning", "label": "2 roles have nobody in them",
             "query": {"search": "label:role has:no-users", "scope": "c1"}},
            {"type": "comments", "severity": "info", "label": "4 comments"}
        ]
    });
    let lines = render::hint_lines(&nest);
    assert_eq!(
        lines,
        vec![
            "- [warning] 2 roles have nobody in them (unassigned_role)".to_string(),
            "    find the rest: nestr search \"label:role has:no-users\" --in c1".to_string(),
            "- [info] 4 comments (comments)".to_string(),
        ]
    );
}

#[test]
fn hint_lines_group_a_listing_by_nest() {
    let list = json!([
        {"_id": "a", "title": "Alpha", "hints": [{"type": "stale_work", "severity": "warning", "label": "Stale"}]},
        {"_id": "b", "title": "Beta", "hints": []}
    ]);
    assert_eq!(
        render::hint_lines(&list),
        vec![
            "Alpha [a]".to_string(),
            "  - [warning] Stale (stale_work)".to_string(),
        ]
    );
}

#[tokio::test]
async fn nest_get_diagnosis_flags_become_query_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .and(query_param("provenance", "true"))
        .and(query_param("rights", "true"))
        .and(query_param("forUser", "u2"))
        .and(query_param("whoCan", "update,delete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"_id": "n1", "title": "x", "rights": {"self": {"update": false}},
                     "whoCan": {"update": []}, "fieldsProvenance": {}}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let diag = nests::Diagnosis {
        provenance: true,
        rights: true,
        for_user: Some("u2".into()),
        who_can: vec!["update".into(), "delete".into()],
    };
    let owned = diag.params("n1").unwrap();
    let params: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let data = nests::fetch_get(&client, "n1", &params).await.unwrap();
    assert_eq!(data["rights"]["self"]["update"], false);
}

#[test]
fn diagnosis_is_for_a_single_nest() {
    let diag = nests::Diagnosis {
        rights: true,
        ..Default::default()
    };
    assert!(diag.params("n1,n2").is_err());
    assert!(nests::Diagnosis::default()
        .params("n1,n2")
        .unwrap()
        .is_empty());
}
