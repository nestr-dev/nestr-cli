use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::activity;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn mine_unwraps_the_activity_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/activity"))
        .and(query_param("limit", "20"))
        .and(query_param("withUser", "u7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"activity": [
                {"date": "2026-10-14T09:30:00.000Z", "workspaceId": "ws1", "nestId": "n1",
                 "nestTitle": "Website", "type": "comment", "description": "commented"}
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, None, Some(20), Some("u7"))
        .await
        .unwrap();
    assert_eq!(data.as_array().unwrap().len(), 1);
    assert_eq!(data[0]["nestTitle"], "Website");
}

#[tokio::test]
async fn another_users_activity_uses_their_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/u9/activity"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"activity": [
                {"date": "2026-10-14T09:30:00.000Z", "nestId": "n2", "nestTitle": "Ops",
                 "type": "consideration", "summary": "Triaged the inbox", "tools": ["search"],
                 "outcome": "done"}
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, Some("u9"), None, None)
        .await
        .unwrap();
    assert_eq!(data[0]["type"], "consideration");
}

#[tokio::test]
async fn a_missing_activity_key_is_an_empty_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/activity"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status": "success", "data": {}})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, None, None, None).await.unwrap();
    assert_eq!(data, json!([]));
}

#[test]
fn a_row_says_what_happened() {
    let consideration = json!({
        "type": "consideration", "summary": "Triaged the inbox",
        "tools": ["search", "update"], "outcome": "done"
    });
    assert_eq!(
        activity::what(&consideration),
        "Triaged the inbox (tools: search, update; outcome: done)"
    );
    let plain = json!({"type": "comment", "description": "<p>commented on it</p>"});
    assert_eq!(activity::what(&plain), "commented on it");
}
