//! The CLI tells the API when a coding agent is driving it, as a fixed name in
//! `X-Client-Agent`. Its own test binary, because it sets a process environment
//! variable and must not race the other tests.

use nestr_cli::api_client::NestrClient;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn sends_the_agent_name_when_an_agent_drives_the_cli() {
    std::env::set_var("NESTR_AGENT_MODE", "1");
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .and(header_exists("x-client-agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"_id": "u1"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let _: serde_json::Value = client.get("/users/me", &[]).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let sent = requests[0]
        .headers
        .get("x-client-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        ["claude-code", "codex", "cursor", "agent"].contains(&sent.as_str()),
        "unexpected agent name: {sent}"
    );
}
