use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::projects;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"p1","title":"Launch","labels":["project"]}],
            "meta":{"page":1,"total_pages":1,"total":1}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = projects::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "p1");
}
