use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::links;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_all_relations_hits_graph_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/graph"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"n2","title":"Mtg","relation":"meeting","direction":"outgoing"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = links::fetch_links(&client, "n1", None, &[]).await.unwrap();
    assert_eq!(data[0]["relation"], "meeting");
}

#[tokio::test]
async fn list_by_relation_hits_relation_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/graph/meeting"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = links::fetch_links(&client, "n1", Some("meeting"), &[])
        .await
        .unwrap();
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn add_posts_target_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/graph/meeting"))
        .and(body_json(serde_json::json!({"targetId":"n2"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"n2","relation":"meeting","status":"created"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = links::add_link(&client, "n1", "meeting", "n2")
        .await
        .unwrap();
    assert_eq!(data[0]["status"], "created");
}

#[tokio::test]
async fn remove_hits_target_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/n1/graph/meeting/n2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"message":"Graph link removed"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = links::remove_link(&client, "n1", "meeting", "n2")
        .await
        .unwrap();
    assert_eq!(data["message"], "Graph link removed");
}
