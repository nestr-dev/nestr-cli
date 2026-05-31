use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::labels;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_workspace_labels() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"l1","title":"urgent"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = labels::fetch_workspace(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "l1");
}

#[tokio::test]
async fn personal_create_posts_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/labels"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"title":"reading"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u1:reading","title":"reading"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"reading"});
    let data = labels::create_personal(&client, &body).await.unwrap();
    assert_eq!(data["title"], "reading");
}
