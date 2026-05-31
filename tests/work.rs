use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::work;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn work_unwraps_projects_and_todos() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/work"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{
                "projects":[{"_id":"p1","title":"Proj"}],
                "todos":[{"_id":"t1","title":"Todo"}],
                "lastUpdateAt":"2026-05-31T00:00:00Z"
            }
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = work::fetch_work(&client, "ws1").await.unwrap();
    assert_eq!(data["projects"][0]["_id"], "p1");
    assert_eq!(data["todos"][0]["_id"], "t1");
}
