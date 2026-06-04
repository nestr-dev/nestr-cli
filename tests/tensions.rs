use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::tensions;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn mine_hits_user_tensions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/tensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t1","title":"Gap","status":"draft"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = tensions::fetch_mine(&client, &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "t1");
}

#[tokio::test]
async fn list_hits_nest_tensions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/c1/tensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t1","title":"Gap"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = tensions::fetch_list(&client, "c1", &[]).await.unwrap();
    assert_eq!(data[0]["title"], "Gap");
}

#[tokio::test]
async fn create_sends_title_and_feeling_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/c1/tensions"))
        .and(body_json(
            serde_json::json!({"title":"Gap","fields":{"tension.feeling":"frustrated"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"t9","title":"Gap"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Gap","fields":{"tension.feeling":"frustrated"}});
    let data = tensions::create_tension(&client, "c1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "t9");
}
