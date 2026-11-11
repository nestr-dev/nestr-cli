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

// ---- Unsupported-route detection: an older Nestr server answers a route it
// doesn't have with its web page (GET) or an empty 405 (writes).

const WEB_PAGE: &str =
    "<!DOCTYPE html>\n<html><head><title>Nestr</title></head><body>app</body></html>";

#[tokio::test]
async fn a_web_page_on_200_says_the_server_lacks_the_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/insights/roles/circles"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(WEB_PAGE, "text/html; charset=utf-8"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/workspaces/ws1/insights/roles/circles", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Unsupported { .. }),
        "{err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("doesn't support `GET /workspaces/ws1/insights/roles/circles`"),
        "{msg}"
    );
    assert!(msg.contains("needs a newer Nestr server"), "{msg}");
    assert!(!msg.contains('<'), "the page must not be printed: {msg}");
}

#[tokio::test]
async fn a_body_starting_with_a_tag_counts_as_a_web_page_whatever_its_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/x"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(format!("  {WEB_PAGE}"), "text/plain"),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me/x", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Unsupported { .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_web_page_on_404_says_the_server_lacks_the_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a/b"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(WEB_PAGE, "text/html"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/a/b", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Unsupported { .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_empty_405_on_a_write_says_the_server_lacks_the_route() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/workspaces/ws1/users/u1/labels/legal_rep"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .put::<serde_json::Value>(
            "/workspaces/ws1/users/u1/labels/legal_rep",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Unsupported { .. }),
        "{err:?}"
    );
    assert!(
        msg.contains("doesn't support `PUT /workspaces/ws1/users/u1/labels/legal_rep`"),
        "{msg}"
    );
    assert!(msg.contains("needs a newer Nestr server"), "{msg}");
}

#[tokio::test]
async fn an_empty_405_on_delete_text_says_the_server_lacks_the_route() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/x/y"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client.delete_text("/x/y").await.unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::Unsupported { .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn other_non_json_replies_name_the_status_and_type_with_a_clean_snippet() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("oops \u{1b}]52;c;Zm9v\u{7}not json\nat all", "text/plain"),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::NonJson { .. }),
        "{err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("unexpected non-JSON reply (HTTP 200, text/plain)"),
        "{msg}"
    );
    assert!(msg.contains("oops not json at all"), "{msg}");
    assert!(!msg.chars().any(|c| c.is_control()), "{msg:?}");
}

#[tokio::test]
async fn a_long_non_json_reply_is_cut_short() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("x".repeat(5000), "text/plain"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let msg = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err()
        .to_string();
    assert!(msg.len() < 300, "{} chars: {msg}", msg.len());
}

#[tokio::test]
async fn a_gateway_web_page_on_5xx_is_not_blamed_on_the_server_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(502).set_body_raw(WEB_PAGE, "text/html"))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        matches!(
            err,
            nestr_cli::error::NestrError::NonJson { status: 502, .. }
        ),
        "{err:?}"
    );
    assert!(msg.contains("HTTP 502, text/html"), "{msg}");
    assert!(!msg.contains('<'), "the page must not be printed: {msg}");
}

#[tokio::test]
async fn a_json_error_body_still_renders_as_before() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"status":"error","message":"boom"})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me", &[])
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "API error (500): boom");
}
