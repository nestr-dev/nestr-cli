use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::users;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_users() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"u1","username":"a@b.c","profile":{"fullName":"A B"}}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = users::fetch_list(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["username"], "a@b.c");
}

#[tokio::test]
async fn add_sends_username_and_profile() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/users"))
        .and(body_json(
            serde_json::json!({"username":"new@b.c","profile":{"fullName":"New One"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u9","username":"new@b.c"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"username":"new@b.c","profile":{"fullName":"New One"}});
    let data = users::add_user(&client, "ws1", &body).await.unwrap();
    assert_eq!(data["_id"], "u9");
}

#[tokio::test]
async fn user_roles_subresource() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users/u1/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"r1","title":"Lead","labels":["role"]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = users::fetch_user_roles(&client, "ws1", "u1", &[])
        .await
        .unwrap();
    assert_eq!(data[0]["title"], "Lead");
}

#[tokio::test]
async fn groups_set_sends_bare_name_array() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/users/u1/groups"))
        .and(body_json(serde_json::json!(["leads", "admins"])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":["leads","admins"]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::set_user_groups(&client, "ws1", "u1", &["leads".into(), "admins".into()])
        .await
        .unwrap();
    assert!(data.is_array());
}

#[tokio::test]
async fn update_sends_profile_full_name_and_email() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/users/u1"))
        .and(body_json(
            serde_json::json!({"profile":{"fullName":"New Name","email":"new@b.c"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u1","username":"new@b.c"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"profile":{"fullName":"New Name","email":"new@b.c"}});
    let data = users::update_user(&client, "ws1", "u1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "u1");
}

#[tokio::test]
async fn list_label_filter_sends_label_query_and_keeps_labels_and_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users"))
        .and(wiremock::matchers::query_param("label", "legal_rep"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"u1","labels":["legal_rep"],
            "fields":{"legal_rep.url":"https://example.com/r/1"}}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = users::fetch_list(&client, "ws1", &[("label", "legal_rep")])
        .await
        .unwrap();
    assert_eq!(data[0]["labels"][0], "legal_rep");
    assert_eq!(
        data[0]["fields"]["legal_rep.url"],
        "https://example.com/r/1"
    );
}

#[tokio::test]
async fn label_add_posts_fields_and_unwraps() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/users/u1/labels/legal_rep"))
        .and(body_json(
            serde_json::json!({"fields":{"url":"https://example.com/r/1","level":3}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"userId":"u1","label":"legal_rep","changed":true,
            "labels":["legal_rep"],"fields":{"legal_rep.url":"https://example.com/r/1"}}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let parsed = nestr_cli::commands::nests::parse_field_args(&[
        "url=https://example.com/r/1".into(),
        "level=3".into(),
    ])
    .unwrap();
    let body = users::fields_body(&parsed).unwrap();
    let data = users::add_user_label(&client, "ws1", "u1", "legal_rep", &body)
        .await
        .unwrap();
    assert_eq!(data["changed"], true);
    assert_eq!(data["label"], "legal_rep");
}

#[tokio::test]
async fn label_set_patches_and_empty_value_clears() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/users/u1/labels/skill_coding"))
        .and(body_json(serde_json::json!({"fields":{"level":""}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"userId":"u1","label":"skill_coding","changed":true,
            "labels":["skill_coding"],"fields":{}}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let parsed = nestr_cli::commands::nests::parse_field_args(&["level=".into()]).unwrap();
    let body = users::fields_body(&parsed).unwrap();
    let data = users::set_user_label_fields(&client, "ws1", "u1", "skill_coding", &body)
        .await
        .unwrap();
    assert_eq!(data["changed"], true);
}

#[tokio::test]
async fn label_remove_sends_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/users/u1/labels/legal_rep"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"userId":"u1","label":"legal_rep","changed":false,
            "labels":[],"fields":{}}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::remove_user_label(&client, "ws1", "u1", "legal_rep")
        .await
        .unwrap();
    assert_eq!(data["changed"], false);
}

#[tokio::test]
async fn label_activity_passes_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/users/u1/labels/activity"))
        .and(wiremock::matchers::query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"a1","verb":"userLabel.added","label":"legal_rep"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::fetch_label_activity(&client, "ws1", "u1", Some(5))
        .await
        .unwrap();
    assert_eq!(data[0]["verb"], "userLabel.added");
}

#[tokio::test]
async fn label_errors_keep_code_and_map_status() {
    let server = MockServer::start().await;
    for (status, code) in [
        (400, "label-not-applicable"),
        (403, "not-authorized"),
        (404, "not-member"),
    ] {
        Mock::given(method("POST"))
            .and(path(format!("/workspaces/ws1/users/u{status}/labels/x")))
            .respond_with(
                ResponseTemplate::new(status).set_body_json(serde_json::json!({
                    "status":"error","error":code,"message":"nope"
                })),
            )
            .mount(&server)
            .await;
    }
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({});
    let e = users::add_user_label(&client, "ws1", "u400", "x", &body)
        .await
        .unwrap_err();
    assert!(
        matches!(e, nestr_cli::error::NestrError::Validation(ref m) if m.contains("label-not-applicable"))
    );
    let e = users::add_user_label(&client, "ws1", "u403", "x", &body)
        .await
        .unwrap_err();
    assert!(
        matches!(e, nestr_cli::error::NestrError::Permission(ref m) if m.contains("not-authorized"))
    );
    let e = users::add_user_label(&client, "ws1", "u404", "x", &body)
        .await
        .unwrap_err();
    assert!(matches!(e, nestr_cli::error::NestrError::NotFound(ref m) if m.contains("not-member")));
}
