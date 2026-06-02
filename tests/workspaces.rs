use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::workspaces;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_workspaces() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"w1","title":"Acme"}],"meta":{"page":1,"total_pages":1,"total":1}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = workspaces::fetch_list(&client, &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "w1");
}

#[tokio::test]
async fn create_sends_title_and_configuration() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces"))
        .and(body_json(
            serde_json::json!({"title":"New","configuration":{"plan":"pro"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"w2","title":"New"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"New","configuration":{"plan":"pro"}});
    let data = workspaces::create_ws(&client, &body).await.unwrap();
    assert_eq!(data["_id"], "w2");
}

#[tokio::test]
async fn set_app_does_read_modify_write_full_array() {
    let server = MockServer::start().await;
    // GET current apps
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/apps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"okr","enabled":false},{"_id":"feedback","enabled":true}]
        })))
        .mount(&server)
        .await;
    // PATCH must receive the FULL array with okr flipped on, feedback unchanged
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/apps"))
        .and(body_json(serde_json::json!([{"_id":"okr","enabled":true},{"_id":"feedback","enabled":true}])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"okr","enabled":true},{"_id":"feedback","enabled":true}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = workspaces::set_app(&client, "ws1", "okr", true)
        .await
        .unwrap();
    assert_eq!(data[0]["enabled"], true);
}

#[tokio::test]
async fn set_app_errors_on_unknown_app() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/apps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"okr","enabled":false}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = workspaces::set_app(&client, "ws1", "nope", true)
        .await
        .unwrap_err();
    assert!(matches!(err, nestr_cli::error::NestrError::NotFound(_)));
}
