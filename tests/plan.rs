use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::plan;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn today_unwraps_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/today"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t1","title":"Focus"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = plan::fetch_today(&client).await.unwrap();
    assert_eq!(data[0]["_id"], "t1");
}

#[tokio::test]
async fn add_labels_now_on_each() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/a/add_label/now"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"a"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = plan::set_now(&client, "a", true).await.unwrap();
    assert_eq!(data["_id"], "a");
}
