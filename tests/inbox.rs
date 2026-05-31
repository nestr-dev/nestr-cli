use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::inbox;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_inbox() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/inbox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"i1","title":"Capture"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = inbox::fetch_list(&client, &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "i1");
}

#[tokio::test]
async fn create_posts_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/inbox"))
        .and(body_json(serde_json::json!({"title":"New"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"i2","title":"New"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = inbox::create_item(&client, "New", None).await.unwrap();
    assert_eq!(data["_id"], "i2");
}

#[tokio::test]
async fn reorder_sends_bare_array() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/users/me/inbox/reorder"))
        .and(body_json(serde_json::json!(["b", "a"])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"b"},{"_id":"a"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = inbox::reorder(&client, &["b".into(), "a".into()])
        .await
        .unwrap();
    assert!(data.is_array());
}
