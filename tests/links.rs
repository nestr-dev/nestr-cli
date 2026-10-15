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

#[tokio::test]
async fn add_depends_on_cycle_explains_direction() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/a/graph/depends_on"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "status":"error","message":"This dependency would create a cycle (a -> b)"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = links::add_link(&client, "a", "depends_on", "b")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("would create a cycle (a -> b)"), "{err}");
    assert!(err.contains("a depends_on b means a waits for b"), "{err}");
}

#[tokio::test]
async fn add_when_link_runs_the_other_way_names_the_remove_command() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/a/graph/meeting"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "status":"error",
            "message":"b already links to a as meeting. Remove that link first to reverse it."
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = links::add_link(&client, "a", "meeting", "b")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("nestr links remove b meeting a"), "{err}");
}

#[tokio::test]
async fn remove_reverse_link_names_the_cli_command() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/a/graph/depends_on/b"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "status":"error",
            "message":"No depends_on link from a to b. The link runs the other way: DELETE /nests/b/graph/depends_on/a"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = links::remove_link(&client, "a", "depends_on", "b")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("runs the other way"), "{err}");
    assert!(err.contains("nestr links remove b depends_on a"), "{err}");
    assert!(!err.contains("DELETE /nests"), "{err}");
}

#[tokio::test]
async fn remove_missing_link_keeps_the_api_message() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/a/graph/meeting/b"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "status":"error","message":"No graph link found from a to b with relation meeting"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = links::remove_link(&client, "a", "meeting", "b")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("No graph link found from a to b"), "{err}");
}
