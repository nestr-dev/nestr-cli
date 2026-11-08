use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::labels;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_workspace_labels() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"l1","title":"urgent"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = labels::fetch_workspace(&client, "ws1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "l1");
}

#[tokio::test]
async fn personal_create_posts_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/me/labels"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"title":"reading"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u1:reading","title":"reading"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"reading"});
    let data = labels::create_personal(&client, &body).await.unwrap();
    assert_eq!(data["title"], "reading");
}

fn opts(applies: &[&str], fields: Option<&str>) -> labels::UserLabelOpts {
    labels::UserLabelOpts {
        applies_to: applies.iter().map(|s| s.to_string()).collect(),
        user_fields: fields.map(String::from),
    }
}

#[tokio::test]
async fn create_posts_applicable_to_and_user_fields() {
    let server = MockServer::start().await;
    let expected = serde_json::json!({
        "title":"Legal rep",
        "applicableTo":["nests","users"],
        "userFields":[{"id":"url","code":"url","title":"Register","type":"url"}]
    });
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/labels"))
        .and(wiremock::matchers::body_json(expected.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"legal_rep","applicableTo":["nests","users"]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let o = opts(
        &["nests", "users"],
        Some(r#"[{"id":"url","code":"url","title":"Register","type":"url"}]"#),
    );
    let body = labels::label_body(Some("Legal rep".into()), None, None, &o, false, false).unwrap();
    assert_eq!(body, expected);
    let data = labels::create_workspace(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "legal_rep");
}

#[tokio::test]
async fn update_patches_and_null_resets() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/labels/legal_rep"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"applicableTo":null,"userFields":null}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"legal_rep"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = labels::label_body(None, None, None, &opts(&[], None), true, true).unwrap();
    let data = labels::update_workspace(&client, "ws1", "legal_rep", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "legal_rep");
}

#[test]
fn user_fields_must_be_a_json_array() {
    assert!(labels::parse_user_fields(r#"{"id":"x"}"#).is_err());
    assert!(labels::parse_user_fields("nope").is_err());
    assert!(labels::parse_user_fields("[]").is_ok());
}

#[tokio::test]
async fn get_returns_applicable_to_and_user_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/labels/legal_rep"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"legal_rep","applicableTo":["users"],
            "userFields":[{"id":"url","code":"url","type":"url"}]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = labels::fetch_workspace_one(&client, "ws1", "legal_rep")
        .await
        .unwrap();
    assert_eq!(data["applicableTo"][0], "users");
    assert_eq!(data["userFields"][0]["code"], "url");
}

#[test]
fn null_list_fields_read_as_empty_instead_of_failing_the_row() {
    let label: nestr_cli::views::LabelView =
        serde_json::from_value(serde_json::json!({"_id": "legal_rep", "applicableTo": null}))
            .unwrap();
    assert!(label.applicable_to.is_empty());
    let user: nestr_cli::views::UserView =
        serde_json::from_value(serde_json::json!({"_id": "u1", "labels": null, "fields": null}))
            .unwrap();
    assert!(user.labels.is_empty() && user.fields.is_empty());
}
