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

#[tokio::test]
async fn create_posts_body_and_returns_nest() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"new1","title":"Task"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Task","parentId":"p1"});
    let data = nests::create_nest(&client, &body).await.unwrap();
    assert_eq!(data["_id"], "new1");
}

#[tokio::test]
async fn bulk_reorder_sends_bare_array_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/reorder"))
        .and(wiremock::matchers::body_json(serde_json::json!([
            "a", "b", "c"
        ])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"a"},{"_id":"b"},{"_id":"c"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::bulk_reorder(&client, "ws1", &["a".into(), "b".into(), "c".into()])
        .await
        .unwrap();
    assert!(data.is_array());
}

#[tokio::test]
async fn delete_hits_delete_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/x"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"message":"deleted","nestId":"x"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::delete_nest(&client, "x").await.unwrap();
    assert_eq!(data["nestId"], "x");
}
