use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::docs;
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn index_lists_the_workspace_context_documents() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files"))
        .and(query_param("context", "nestradamus_files"))
        .and(query_param_is_missing("search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": [{"id": "d1", "name": "constitution.pdf", "size": 52000,
                      "description": "How governance works here."}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = docs::list(&client, "ws1", None).await.unwrap();
    assert_eq!(data[0]["description"], "How governance works here.");
}

#[tokio::test]
async fn search_returns_the_matching_passages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files"))
        .and(query_param("context", "nestradamus_files"))
        .and(query_param("search", "parental leave"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": [{"id": "d2", "name": "handbook.pdf",
                      "matches": [{"excerpt": "Parental leave is 16 weeks", "offset": 1200}]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = docs::list(&client, "ws1", Some("parental leave"))
        .await
        .unwrap();
    assert_eq!(data[0]["matches"][0]["offset"], 1200);
}

#[tokio::test]
async fn read_all_follows_next_offset_to_the_end() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files/d1"))
        .and(query_param("as", "text"))
        .and(query_param_is_missing("offset"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "d1", "name": "c.pdf", "text": "first half, ", "offset": 0,
                     "total": 23, "nextOffset": 12}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files/d1"))
        .and(query_param("as", "text"))
        .and(query_param("offset", "12"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "d1", "name": "c.pdf", "text": "second half", "offset": 12,
                     "total": 23, "nextOffset": null}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = docs::read_all(&client, "ws1", "d1", None).await.unwrap();
    assert_eq!(data["text"], "first half, second half");
    assert_eq!(data["offset"], 0);
    assert!(data["nextOffset"].is_null());
}

#[tokio::test]
async fn read_all_stops_when_the_offset_does_not_advance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files/d1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "d1", "name": "c.pdf", "text": "", "offset": 5, "total": 10, "nextOffset": 5}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = docs::read_all(&client, "ws1", "d1", Some(5))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("did not advance"), "{err}");
}
