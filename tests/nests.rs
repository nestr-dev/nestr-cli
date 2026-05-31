use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::nests;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_single_nest_unwraps_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n1","title":"Root"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::fetch_get(&client, "n1", &[]).await.unwrap();
    assert_eq!(data["_id"], "n1");
}

#[tokio::test]
async fn get_multiple_ids_comma_joined() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/a,b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"a"},{"_id":"b"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::fetch_get(&client, "a,b", &[]).await.unwrap();
    assert!(data.is_array());
}

#[tokio::test]
async fn children_unwraps_array_with_meta() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/children"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"c1","title":"Child"}],
            "meta":{"page":1,"total_pages":1,"total":1}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = nests::fetch_children(&client, "n1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "c1");
    assert_eq!(meta.unwrap()["total"], 1);
}
