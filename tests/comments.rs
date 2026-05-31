use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::comments;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_hits_posts_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/posts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"c1","body":"hi","createdBy":"u1"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = comments::fetch_list(&client, "n1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "c1");
}

#[tokio::test]
async fn add_posts_body_with_parent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/posts"))
        .and(body_json(
            serde_json::json!({"body":"hello","parentId":"n1"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"c9","body":"hello"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = comments::add_comment(&client, "n1", "hello", &[])
        .await
        .unwrap();
    assert_eq!(data["_id"], "c9");
}

#[tokio::test]
async fn edit_patches_nest_with_title() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/c9"))
        .and(body_json(serde_json::json!({"title":"edited"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"c9","title":"edited"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = comments::edit_comment(&client, "c9", "edited")
        .await
        .unwrap();
    assert_eq!(data["title"], "edited");
}
