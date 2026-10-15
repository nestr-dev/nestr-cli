use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::queues;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_queues() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/queues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success",
            "data":[{"key":"support","title":"Support","subscribed":true}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = queues::fetch_list(&client).await.unwrap();
    assert_eq!(data[0]["key"], "support");
    assert_eq!(data[0]["subscribed"], true);
}

#[tokio::test]
async fn threads_pass_unread_filter_and_keep_the_envelope_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/queues/support/threads"))
        .and(query_param("unread", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success",
            "data":{"key":"support","subscribed":false,"threads":[{"id":"t1","title":"Login fails"}]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/me/queues/support/threads"))
        .and(query_param_is_missing("unread"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success",
            "data":{"key":"support","subscribed":false,"threads":[]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = queues::fetch_threads(&client, "support", &[("unread", "true")])
        .await
        .unwrap();
    assert_eq!(data["subscribed"], false);
    assert_eq!(data["threads"][0]["id"], "t1");
    let all = queues::fetch_threads(&client, "support", &[])
        .await
        .unwrap();
    assert_eq!(all["threads"].as_array().unwrap().len(), 0);
}
