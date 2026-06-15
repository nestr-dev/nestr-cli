//! SEC-2 / SEC-16 regression: a crafted id must not retarget the request.
use nestr_cli::api_client::NestrClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn traversal_id_is_rejected_before_any_request() {
    let server = MockServer::start().await;
    // If the guard fails, this mock would catch the retargeted DELETE.
    Mock::given(method("DELETE"))
        .and(path("/workspaces/W/webhooks/X"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = client
        .delete::<serde_json::Value>("/nests/../workspaces/W/webhooks/X")
        .await
        .unwrap_err();
    assert!(matches!(err, nestr_cli::error::NestrError::Validation(_)));
}
