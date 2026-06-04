use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::tensions;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn mine_hits_user_tensions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/tensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t1","title":"Gap","status":"draft"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = tensions::fetch_mine(&client, &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "t1");
}

#[tokio::test]
async fn list_hits_nest_tensions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/c1/tensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t1","title":"Gap"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = tensions::fetch_list(&client, "c1", &[]).await.unwrap();
    assert_eq!(data[0]["title"], "Gap");
}

#[tokio::test]
async fn create_sends_title_and_feeling_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/c1/tensions"))
        .and(body_json(
            serde_json::json!({"title":"Gap","fields":{"tension.feeling":"frustrated"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"t9","title":"Gap"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Gap","fields":{"tension.feeling":"frustrated"}});
    let data = tensions::create_tension(&client, "c1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "t9");
}

#[tokio::test]
async fn parts_add_sends_label_and_accountabilities() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/c1/tensions/t1/parts"))
        .and(body_json(serde_json::json!({
            "title":"Treasurer","labels":["role"],"accountabilities":["Manage the books"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"p9","title":"Treasurer"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Treasurer","labels":["role"],"accountabilities":["Manage the books"]});
    let data = tensions::add_part(&client, "c1", "t1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "p9");
}

#[tokio::test]
async fn propose_update_patches_parts_collection_with_id() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/c1/tensions/t1/parts"))
        .and(body_json(
            serde_json::json!({"_id":"role7","purpose":"New purpose"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"p10"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"_id":"role7","purpose":"New purpose"});
    let data = tensions::propose_update(&client, "c1", "t1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "p10");
}

#[tokio::test]
async fn propose_delete_sends_id_in_delete_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/c1/tensions/t1/parts"))
        .and(body_json(serde_json::json!({"_id":"role7"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"p11"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = tensions::propose_delete(&client, "c1", "t1", "role7")
        .await
        .unwrap();
    assert_eq!(data["_id"], "p11");
}

#[tokio::test]
async fn changes_hits_part_changes_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/c1/tensions/t1/parts/p1/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"variable":"role.title","newValue":"X","oldValue":null}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = tensions::fetch_changes(&client, "c1", "t1", "p1")
        .await
        .unwrap();
    assert_eq!(data[0]["variable"], "role.title");
}

#[tokio::test]
async fn child_add_posts_title_and_label() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/c1/tensions/t1/parts/p1/children"))
        .and(body_json(
            serde_json::json!({"title":"Keep records","labels":["accountability"]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"ch9","title":"Keep records"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Keep records","labels":["accountability"]});
    let data = tensions::add_child(&client, "c1", "t1", "p1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "ch9");
}

#[tokio::test]
async fn set_status_patches_status_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/c1/tensions/t1/status"))
        .and(body_json(serde_json::json!({"status":"proposed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"status":"proposed"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = tensions::set_status(&client, "c1", "t1", "proposed")
        .await
        .unwrap();
    assert_eq!(data["status"], "proposed");
}

#[tokio::test]
async fn awaiting_consent_hits_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/tensions/awaiting-my-consent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"t5","title":"Vote me"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = tensions::fetch_awaiting(&client, &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "t5");
}

#[tokio::test]
async fn child_update_patches_child_path() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/c1/tensions/t1/parts/p1/children/ch1"))
        .and(body_json(serde_json::json!({"title":"Renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"ch1","title":"Renamed"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Renamed"});
    let data = tensions::update_child(&client, "c1", "t1", "p1", "ch1", &body)
        .await
        .unwrap();
    assert_eq!(data["title"], "Renamed");
}

#[tokio::test]
async fn child_delete_hits_child_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/c1/tensions/t1/parts/p1/children/ch1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"message":"Child deleted","childId":"ch1"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = tensions::delete_child(&client, "c1", "t1", "p1", "ch1")
        .await
        .unwrap();
    assert_eq!(data["childId"], "ch1");
}
