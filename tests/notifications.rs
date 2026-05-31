use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::notifications;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_passes_skip_and_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/notifications"))
        .and(query_param("limit", "10"))
        .and(query_param("skip", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"x","group":"mentions","title":"Hi"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = notifications::fetch_list(&client, &[("limit", "10"), ("skip", "20")])
        .await
        .unwrap();
    assert_eq!(data[0]["group"], "mentions");
}

#[tokio::test]
async fn read_marks_all() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/notifications/mark-all-read"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"markedCount":3}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = notifications::mark_read(&client).await.unwrap();
    assert_eq!(data["markedCount"], 3);
}
