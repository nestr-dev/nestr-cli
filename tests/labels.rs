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
    let body =
        labels::label_body(Some("Legal rep".into()), None, None, None, &o, false, false).unwrap();
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
    let body = labels::label_body(None, None, None, None, &opts(&[], None), true, true).unwrap();
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

// POST stores the body as the label document, which keeps colour and icon under
// `properties`; a top-level `color` would be dropped. PATCH takes them as shortcuts.
#[tokio::test]
async fn create_nests_color_and_icon_under_properties() {
    let server = MockServer::start().await;
    let expected = serde_json::json!({
        "_id": "review",
        "title": "Review",
        "group": "flow",
        "properties": {"color": "#ff5733", "icon": "eye"}
    });
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/labels"))
        .and(wiremock::matchers::body_json(expected.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"review","title":"Review"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = labels::create_body(
        "Review".into(),
        Some("review".into()),
        Some("#ff5733".into()),
        Some("eye".into()),
        Some("flow".into()),
        &opts(&[], None),
    )
    .unwrap();
    assert_eq!(body, expected);
    let data = labels::create_workspace(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "review");
}

#[test]
fn create_without_color_or_icon_sends_no_properties() {
    let body =
        labels::create_body("Review".into(), None, None, None, None, &opts(&[], None)).unwrap();
    assert_eq!(body, serde_json::json!({"title": "Review"}));
}

#[test]
fn update_keeps_color_icon_shortcuts_and_sends_group() {
    let body = labels::label_body(
        None,
        Some("#00ff00".into()),
        Some("eye".into()),
        Some("flow".into()),
        &opts(&[], None),
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        body,
        serde_json::json!({"color": "#00ff00", "icon": "eye", "group": "flow"})
    );
}

#[tokio::test]
async fn delete_workspace_label_returns_the_deleted_id() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/labels/review"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"review","deleted":true}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = labels::delete_workspace(&client, "ws1", "review")
        .await
        .unwrap();
    assert_eq!(data["_id"], "review");
    assert_eq!(data["deleted"], true);
}

// A single-label read keeps colour and icon under `display`, not at the top level.
#[test]
fn label_view_reads_color_and_icon_from_display() {
    let label: nestr_cli::views::LabelView = serde_json::from_value(serde_json::json!({
        "_id": "review", "display": {"color": "#ff5733", "icon": "eye"}
    }))
    .unwrap();
    assert_eq!(label.shown_color(), "#ff5733");
    assert_eq!(label.shown_icon(), "eye");
    let flat: nestr_cli::views::LabelView =
        serde_json::from_value(serde_json::json!({"_id": "x", "color": "#000000"})).unwrap();
    assert_eq!(flat.shown_color(), "#000000");
    assert_eq!(flat.shown_icon(), "");
}

// --read-only must stop the delete before any request: expect(0) fails the test if
// the DELETE fires. An api-key file profile keeps client resolution off the network.
#[tokio::test]
async fn read_only_blocks_label_delete_before_writing() {
    use nestr_cli::commands::GlobalArgs;
    use nestr_cli::config::{save_profile, AuthKind, CredentialStorage, Profile};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    let profile = Profile {
        auth: AuthKind::ApiKey,
        credential_storage: CredentialStorage::File,
        host: server.uri(),
        workspace_id: "ws".into(),
        api_key: Some("k".into()),
        oauth_client_id: None,
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: None,
        client_instance_id: None,
        default_output_format: None,
    };
    save_profile("ro", &profile).unwrap();

    let g = GlobalArgs {
        profile: Some("ro".into()),
        read_only: true,
        yes: true,
        ..Default::default()
    };
    let cmd = labels::LabelsCmd::Delete {
        label_id: "review".into(),
    };
    let err = labels::run(cmd, &g).await.unwrap_err();
    assert!(
        err.to_string().contains("read-only"),
        "expected a read-only error, got: {err}"
    );

    std::env::remove_var("NESTR_HOME");
}
