use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::me::fetch_me;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_me_returns_user_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        // The live API wraps /users/me in {status, data}; fetch_me must unwrap it.
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "data": {"_id":"u1","username":"a@b.c","profile":{"fullName":"A B"}}
        })))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let me = fetch_me(&client).await.unwrap();
    assert_eq!(me["username"], "a@b.c");
    assert_eq!(me["profile"]["fullName"], "A B");
}

#[test]
fn user_labels_line_lists_labels_for_the_workspace() {
    use nestr_cli::commands::me::user_labels_line;
    let me = serde_json::json!({"userLabels":{"ws1":{"labels":["legal_rep","skill_coding"],"fields":{}}}});
    assert_eq!(
        user_labels_line(&me, "ws1").as_deref(),
        Some("labels: legal_rep, skill_coding")
    );
    assert_eq!(user_labels_line(&me, "ws2"), None);
    assert_eq!(user_labels_line(&serde_json::json!({}), "ws1"), None);
}
