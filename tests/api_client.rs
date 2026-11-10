use nestr_cli::api_client::NestrClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_sends_bearer_and_consumer_and_parses_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .and(header("authorization", "Bearer tok-123"))
        .and(header("x-client-consumer", "nestr-cli"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"_id":"u1"})))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok-123").unwrap();
    let v: serde_json::Value = client.get("/users/me", &[]).await.unwrap();
    assert_eq!(v["_id"], "u1");
}

#[tokio::test]
async fn forbidden_maps_to_permission_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(serde_json::json!({"description":"nope"})),
        )
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, nestr_cli::error::NestrError::Permission(_)));
}

#[tokio::test]
async fn not_found_maps_to_not_found_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(serde_json::json!({"description":"missing"})),
        )
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, nestr_cli::error::NestrError::NotFound(_)));
}

#[tokio::test]
async fn patch_handles_empty_204_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/x"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let v: serde_json::Value = client
        .patch("/nests/x", &serde_json::json!({"a":1}))
        .await
        .unwrap();
    assert_eq!(v, serde_json::json!({}));
}

#[tokio::test]
async fn delete_body_sends_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/x/tensions/t/parts"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"_id":"r1"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status":"ok","data":{"ok":true}})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let v: serde_json::Value = client
        .delete_body(
            "/nests/x/tensions/t/parts",
            &serde_json::json!({"_id":"r1"}),
        )
        .await
        .unwrap();
    assert_eq!(v["status"], "ok");
}

#[tokio::test]
async fn delete_text_returns_raw_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws/webhooks/wh1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("success"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = client
        .delete_text("/workspaces/ws/webhooks/wh1")
        .await
        .unwrap();
    assert_eq!(body, "success");
}

// A permission refusal the server throws (a write guard's) now answers 403, but with
// the framework's plain-text "Server error." body in production. It must read as a
// refusal, not as a server fault, and keep the 403 "permission denied" framing.
#[tokio::test]
async fn thrown_403_with_a_plain_server_error_body_reads_as_a_refusal() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("content-type", "text/html")
                .set_body_string("Server error."),
        )
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .patch::<serde_json::Value>("/nests/n1", &serde_json::json!({"title":"x"}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Permission(_)),
        "{err:?}"
    );
    let msg = err.to_string();
    assert!(!msg.contains("Server error"), "{msg}");
    assert!(msg.contains("refused"), "{msg}");
}

#[tokio::test]
async fn a_403_with_a_detail_keeps_the_servers_words() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(serde_json::json!({"description":"You may not edit this role"})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "permission denied: You may not edit this role"
    );
}
