use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::insights;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_hits_insights_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"type":"roles","title":"Roles","currentValue":12}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = insights::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["type"], "roles");
}

#[tokio::test]
async fn get_hits_metric_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"type":"roles","title":"Roles","currentValue":12}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = insights::fetch_get(&client, "ws1", "roles").await.unwrap();
    assert_eq!(data["currentValue"], 12);
}

#[tokio::test]
async fn history_passes_from_param() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/roles/history"))
        .and(query_param("from", "2026-01-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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
