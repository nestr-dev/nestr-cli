use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::users;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_users() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"u1","username":"a@b.c","profile":{"fullName":"A B"}}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = users::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["username"], "a@b.c");
}

#[tokio::test]
async fn add_sends_username_and_profile() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/users"))
        .and(body_json(
            serde_json::json!({"username":"new@b.c","profile":{"fullName":"New One"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u9","username":"new@b.c"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"username":"new@b.c","profile":{"fullName":"New One"}});
    let data = users::add_user(&client, "ws1", &body).await.unwrap();
    assert_eq!(data["_id"], "u9");
}

#[tokio::test]
async fn user_roles_subresource() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users/u1/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"r1","title":"Lead","labels":["role"]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = users::fetch_user_roles(&client, "ws1", "u1", &[])
        .await
        .unwrap();
    assert_eq!(data[0]["title"], "Lead");
}

#[tokio::test]
async fn groups_set_sends_bare_name_array() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/users/u1/groups"))
        .and(body_json(serde_json::json!(["leads", "admins"])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":["leads","admins"]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::set_user_groups(&client, "ws1", "u1", &["leads".into(), "admins".into()])
        .await
        .unwrap();
    assert!(data.is_array());
}
