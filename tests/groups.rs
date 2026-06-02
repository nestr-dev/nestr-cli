use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::groups;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_groups() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/groups"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"g1","name":"leads"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = groups::fetch_list(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["name"], "leads");
}

#[tokio::test]
async fn create_sends_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/groups"))
        .and(body_json(serde_json::json!({"name":"marketing"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"ws1:marketing","name":"marketing"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = groups::create_group(&client, "ws1", "marketing")
        .await
        .unwrap();
    assert_eq!(data["name"], "marketing");
}
