use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::roles;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_roles() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"r1","title":"Treasurer","labels":["role"]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = roles::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["title"], "Treasurer");
}

#[tokio::test]
async fn create_sends_parent_and_accountabilities() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/roles"))
        .and(body_json(serde_json::json!({
            "title":"Treasurer","parentId":"c1","accountabilities":["Manage the books"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"r9","title":"Treasurer"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Treasurer","parentId":"c1","accountabilities":["Manage the books"]});
    let data = roles::create_role(&client, "ws1", &body).await.unwrap();
    assert_eq!(data["_id"], "r9");
}

#[tokio::test]
async fn update_patches_role() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/roles/r9"))
        .and(body_json(
            serde_json::json!({"purpose":"Keep the money safe"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"r9","purpose":"Keep the money safe"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"purpose":"Keep the money safe"});
    let data = roles::update_role(&client, "ws1", "r9", &body)
        .await
        .unwrap();
    assert_eq!(data["purpose"], "Keep the money safe");
}
