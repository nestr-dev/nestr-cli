use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::me::fetch_me;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_me_returns_user_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"_id":"u1","username":"a@b.c","profile":{"fullName":"A B"}}),
        ))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let me = fetch_me(&client).await.unwrap();
    assert_eq!(me["username"], "a@b.c");
}
