use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::circles;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_circles() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/circles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"c1","title":"General","labels":["anchor-circle"]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = circles::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "c1");
}

#[tokio::test]
async fn create_sends_accountabilities_and_domains() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/circles"))
        .and(body_json(serde_json::json!({
            "title":"Marketing","accountabilities":["Run campaigns"],"domains":["The blog"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"c9","title":"Marketing"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Marketing","accountabilities":["Run campaigns"],"domains":["The blog"]});
    let data = circles::create_circle(&client, "ws1", &body).await.unwrap();
    assert_eq!(data["_id"], "c9");
}

#[tokio::test]
async fn roles_subresource_hits_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/circles/c1/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"r1","title":"Lead","labels":["role"]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = circles::fetch_roles(&client, "ws1", "c1", &[])
        .await
        .unwrap();
    assert_eq!(data[0]["title"], "Lead");
}

#[tokio::test]
async fn posts_subresource_hits_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/circles/c1/posts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"p1","body":"hi"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = circles::fetch_posts(&client, "ws1", "c1", &[])
        .await
        .unwrap();
    assert_eq!(data[0]["_id"], "p1");
}

#[tokio::test]
async fn update_replaces_accountabilities_and_domains() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/circles/c1"))
        .and(body_json(serde_json::json!({
            "accountabilities":["New acc"],"domains":["New domain"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"c1","title":"General"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"accountabilities":["New acc"],"domains":["New domain"]});
    let data = circles::update_circle(&client, "ws1", "c1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "c1");
}
