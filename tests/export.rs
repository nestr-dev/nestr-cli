use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::export;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn governance_unwraps_dump() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/governance"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"users":[],"circles":[{"_id":"c1"}],"roles":[],"policies":[]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = export::fetch_governance(&client, "ws1").await.unwrap();
    assert_eq!(data["circles"][0]["_id"], "c1");
}

#[tokio::test]
async fn work_unwraps_dump() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/work"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"projects":[],"todos":[{"_id":"t1"}],"lastUpdateAt":"x"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = export::fetch_work(&client, "ws1").await.unwrap();
    assert_eq!(data["todos"][0]["_id"], "t1");
}
