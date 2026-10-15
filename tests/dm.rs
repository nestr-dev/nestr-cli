use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::dm;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ok(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"success","data":data}))
}

#[tokio::test]
async fn list_passes_filters_and_unwraps_threads() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/dm"))
        .and(query_param("user", "alex"))
        .and(query_param("unread", "true"))
        .and(query_param("includeCompleted", "true"))
        .respond_with(ok(serde_json::json!([
            {"id":"t1","title":"Hello","users":["u1","u2"],"completed":false,"lastPostAt":"2026-10-01T10:00:00Z"}
        ])))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = dm::fetch_list(
        &client,
        &[
            ("user", "alex"),
            ("unread", "true"),
            ("includeCompleted", "true"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(data[0]["id"], "t1");
}

#[tokio::test]
async fn start_posts_user_and_optional_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/dm"))
        .and(body_json(serde_json::json!({"user":"alex"})))
        .respond_with(ok(serde_json::json!({"id":"t2","title":"15 Oct"})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/users/me/dm"))
        .and(body_json(
            serde_json::json!({"user":"alex","title":"Budget"}),
        ))
        .respond_with(ok(serde_json::json!({"id":"t3","title":"Budget"})))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let a = dm::start_thread(&client, "alex", None).await.unwrap();
    assert_eq!(a["id"], "t2");
    let b = dm::start_thread(&client, "alex", Some("Budget"))
        .await
        .unwrap();
    assert_eq!(b["id"], "t3");
}

#[tokio::test]
async fn show_embeds_unread_posts_when_asked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/dm/t1"))
        .and(query_param("unread", "true"))
        .respond_with(ok(serde_json::json!({
            "_id":"t1","title":"Hello","posts":[{"_id":"p1","body":"new"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/me/dm/t1"))
        .and(query_param_is_missing("unread"))
        .respond_with(ok(serde_json::json!({"_id":"t1","title":"Hello"})))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let with = dm::fetch_thread(&client, "t1", &[("unread", "true")])
        .await
        .unwrap();
    assert_eq!(with["posts"][0]["_id"], "p1");
    let bare = dm::fetch_thread(&client, "t1", &[]).await.unwrap();
    assert!(bare.get("posts").is_none());
}

#[tokio::test]
async fn posts_pass_read_filter_and_depth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/dm/t1/posts"))
        .and(query_param("unread", "false"))
        .and(query_param("depth", "all"))
        .respond_with(ok(serde_json::json!([{"_id":"p1","body":"old"}])))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = dm::fetch_posts(&client, "t1", &[("unread", "false"), ("depth", "all")])
        .await
        .unwrap();
    assert_eq!(data[0]["body"], "old");
}

#[tokio::test]
async fn post_sends_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/dm/t1/posts"))
        .and(body_json(serde_json::json!({"body":"hi there"})))
        .respond_with(ok(serde_json::json!({"_id":"p9","body":"hi there"})))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = dm::post_message(&client, "t1", "hi there").await.unwrap();
    assert_eq!(data["_id"], "p9");
}

#[tokio::test]
async fn close_and_reopen_send_completed_true_and_null() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/users/me/dm/t1"))
        .and(body_json(serde_json::json!({"completed":true})))
        .respond_with(ok(serde_json::json!({"_id":"t1","completed":true})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/users/me/dm/t1"))
        .and(body_json(serde_json::json!({"completed":null})))
        .respond_with(ok(serde_json::json!({"_id":"t1","completed":false})))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let closed = dm::update_thread(&client, "t1", &dm::close_body())
        .await
        .unwrap();
    assert_eq!(closed["completed"], true);
    let reopened = dm::update_thread(&client, "t1", &dm::reopen_body())
        .await
        .unwrap();
    assert_eq!(reopened["completed"], false);
}

#[tokio::test]
async fn update_sends_only_the_keys_given() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/users/me/dm/t1"))
        .and(body_json(
            serde_json::json!({"title":"Renamed","users":["u1","u3"]}),
        ))
        .respond_with(ok(serde_json::json!({"_id":"t1","title":"Renamed"})))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let users = vec!["u1".to_string(), "u3".to_string()];
    let body = dm::update_body(Some("Renamed"), Some(&users)).unwrap();
    let data = dm::update_thread(&client, "t1", &body).await.unwrap();
    assert_eq!(data["title"], "Renamed");
}

#[test]
fn update_with_nothing_to_change_is_refused() {
    assert!(dm::update_body(None, None).is_err());
}

#[tokio::test]
async fn mark_read_posts_to_generic_post_route() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/posts/p1/read"))
        .respond_with(ok(
            serde_json::json!({"key":"t1","readAt":"2026-10-15T10:00:00Z"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = dm::mark_read(&client, "p1").await.unwrap();
    assert_eq!(data["key"], "t1");
}

#[tokio::test]
async fn update_warnings_are_collected_as_notices() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/users/me/dm/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success",
            "data":{"_id":"t1","title":"Renamed"},
            "warnings":[{"message":"Ignored unknown key: colour"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = dm::update_body(Some("Renamed"), None).unwrap();
    let data = dm::update_thread(&client, "t1", &body).await.unwrap();
    assert_eq!(data["title"], "Renamed");
    let notices = client.notices();
    assert_eq!(
        notices[0]["warnings"][0]["message"],
        "Ignored unknown key: colour"
    );
}
