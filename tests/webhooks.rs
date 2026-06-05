use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::webhooks;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_passes_through_bare_array() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/webhooks"))
        // webhooks list is a BARE ARRAY, not {status,data}
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"_id":"wh1","url":"https://x.test/h","type":"nest","event":"create"}
        ])))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = webhooks::fetch_list(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["_id"], "wh1");
}

#[tokio::test]
async fn get_unwraps_single_element_array() {
    let server = MockServer::start().await;
    // get may return data as a single-element array (route .fetch() quirk)
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/webhooks/wh1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"wh1","url":"https://x.test/h"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = webhooks::fetch_get(&client, "ws1", "wh1").await.unwrap();
    assert_eq!(data["_id"], "wh1"); // unwrapped to the object, not an array
}

#[tokio::test]
async fn create_sends_url_type_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/webhooks"))
        .and(body_json(
            serde_json::json!({"url":"https://x.test/h","type":"nest","event":"create"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"wh9","url":"https://x.test/h"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"url":"https://x.test/h","type":"nest","event":"create"});
    let data = webhooks::create_webhook(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "wh9");
}

#[tokio::test]
async fn delete_tolerates_bare_success() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/webhooks/wh1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("success"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = webhooks::delete_webhook(&client, "ws1", "wh1")
        .await
        .unwrap();
    assert_eq!(body, "success");
}
