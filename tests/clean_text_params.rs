//! COR-12: text-mode parts list / changes must send cleanText=true.
use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::tensions;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn parts_list_sends_clean_text() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/tensions/t1/parts"))
        .and(query_param("cleanText", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    tensions::fetch_parts(&client, "n1", "t1", &[("cleanText", "true")])
        .await
        .unwrap();
}

#[tokio::test]
async fn changes_sends_clean_text() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/tensions/t1/parts/p1/changes"))
        .and(query_param("cleanText", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    tensions::fetch_changes(&client, "n1", "t1", "p1", &[("cleanText", "true")])
        .await
        .unwrap();
}
