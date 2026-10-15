use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::connectors::{self, ConnectorSpec, OwnerType};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn empty_spec() -> ConnectorSpec {
    ConnectorSpec {
        type_: None,
        config: None,
        capabilities: None,
        exposure: None,
        auth_strategy: None,
    }
}

#[tokio::test]
async fn list_unwraps_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/connectors"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":[{"_id":"k1","name":"Xero","type":"mcp","authStrategy":"oauth2",
                     "config":{"url":"https://mcp.xero.test"},"enabled":true}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::fetch_list(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["_id"], "k1");
}

#[tokio::test]
async fn templates_keep_hints() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/connector-templates"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":[{"id":"xero","name":"Xero","category":"finance","type":"mcp","blurb":"Accounting"}],
            "hints":[{"type":"connector_template_create","label":"Register one with templateId."}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, hints) = connectors::fetch_templates(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["id"], "xero");
    assert_eq!(hints, vec!["Register one with templateId."]);
}

#[test]
fn register_body_from_template_sends_only_template_and_name() {
    let body =
        connectors::register_body(Some("xero".into()), Some("Books".into()), empty_spec()).unwrap();
    assert_eq!(body, json!({"templateId":"xero","name":"Books"}));
}

#[test]
fn register_body_by_hand_needs_type_and_name() {
    assert!(connectors::register_body(None, Some("X".into()), empty_spec()).is_err());
    let mut spec = empty_spec();
    spec.type_ = Some("mcp".into());
    assert!(connectors::register_body(None, None, spec).is_err());
}

#[test]
fn register_body_by_hand_parses_json_flags() {
    let spec = ConnectorSpec {
        type_: Some("mcp".into()),
        config: Some(r#"{"url":"https://mcp.example.test"}"#.into()),
        capabilities: Some(r#"{"discover":true}"#.into()),
        exposure: Some(r#"{"domainGated":true}"#.into()),
        auth_strategy: Some("oauth2".into()),
    };
    let body = connectors::register_body(None, Some("Example".into()), spec).unwrap();
    assert_eq!(
        body,
        json!({
            "type":"mcp","name":"Example",
            "config":{"url":"https://mcp.example.test"},
            "capabilities":{"discover":true},
            "exposure":{"domainGated":true},
            "authStrategy":"oauth2"
        })
    );
}

#[test]
fn register_body_rejects_bad_json() {
    let spec = ConnectorSpec {
        type_: Some("mcp".into()),
        config: Some("url=https://x".into()),
        ..empty_spec()
    };
    let err = connectors::register_body(None, Some("X".into()), spec).unwrap_err();
    assert!(err.to_string().contains("--config"));
}

#[tokio::test]
async fn register_posts_and_keeps_hints() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/connectors"))
        .and(body_json(json!({"templateId":"xero"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"_id":"k2","name":"Xero","type":"mcp"},
            "hints":[{"label":"Bind it to a role next."}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = connectors::register_body(Some("xero".into()), None, empty_spec()).unwrap();
    let (data, hints) = connectors::register_connector(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "k2");
    assert_eq!(hints, vec!["Bind it to a role next."]);
}

#[test]
fn spec_body_sends_enabled_and_name() {
    let body = connectors::spec_body(Some("Books".into()), empty_spec(), Some(false)).unwrap();
    assert_eq!(
        serde_json::Value::Object(body),
        json!({"name":"Books","enabled":false})
    );
    assert!(connectors::spec_body(None, empty_spec(), None)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn update_patches_connector() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/connectors/k1"))
        .and(body_json(json!({"enabled":false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"k1","enabled":false}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::update_connector(&client, "ws1", "k1", &json!({"enabled":false}))
        .await
        .unwrap();
    assert_eq!(data["enabled"], false);
}

#[tokio::test]
async fn remove_deletes_connector() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/connectors/k1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"success","data":{"_id":"k1"}})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::remove_connector(&client, "ws1", "k1")
        .await
        .unwrap();
    assert_eq!(data["_id"], "k1");
}

#[test]
fn bind_body_nests_owner() {
    assert_eq!(
        connectors::bind_body("k1", OwnerType::RoleDomain, "d1"),
        json!({"connectorId":"k1","owner":{"type":"role-domain","id":"d1"}})
    );
}

#[tokio::test]
async fn bind_posts_connection_and_returns_domain() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/connections"))
        .and(body_json(
            json!({"connectorId":"k1","owner":{"type":"role","id":"r1"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"_id":"c1","owner":{"type":"role-domain","id":"d1"},
                    "domainId":"d1","domainCreated":true}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = connectors::bind_body("k1", OwnerType::Role, "r1");
    let data = connectors::bind_connector(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["domainId"], "d1");
}

#[tokio::test]
async fn connections_list_hits_list_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/connections/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":[{"connectionId":"c1","connectorName":"Xero",
                     "owner":{"type":"role-domain","id":"d1"},"ownerLabel":"Bookkeeper (Finance) / Books",
                     "connectedCount":1,"authorizations":[]}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::fetch_connections(&client, "ws1", false)
        .await
        .unwrap();
    assert_eq!(data[0]["connectionId"], "c1");
}

#[tokio::test]
async fn connections_list_include_disabled_sends_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/connections/list"))
        .and(query_param("includeDisabled", "true"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status":"success","data":[]})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::fetch_connections(&client, "ws1", true)
        .await
        .unwrap();
    assert_eq!(data, json!([]));
}

#[tokio::test]
async fn connect_link_is_a_post_returning_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/connections/c1/connect-link"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"connectionId":"c1","url":"https://app.test/n/ws1?connect=c1"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::connect_link(&client, "ws1", "c1")
        .await
        .unwrap();
    assert_eq!(data["url"], "https://app.test/n/ws1?connect=c1");
}

#[tokio::test]
async fn remove_connection_returns_revoked_count() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/connections/c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"c1","revokedCount":2}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = connectors::remove_connection(&client, "ws1", "c1")
        .await
        .unwrap();
    assert_eq!(data["revokedCount"], 2);
}

#[tokio::test]
async fn revoke_handles_both_response_shapes() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/connections/c1/authorizations"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"success","data":{"_id":"auth1"}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/workspaces/ws1/connections/c2/authorizations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"revoked":true,"vaultCleanupFailed":true,"message":"vault unreachable"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let ok = connectors::revoke_credential(&client, "ws1", "c1")
        .await
        .unwrap();
    assert_eq!(ok["_id"], "auth1");
    let partial = connectors::revoke_credential(&client, "ws1", "c2")
        .await
        .unwrap();
    assert_eq!(partial["vaultCleanupFailed"], true);
}

// --read-only must stop a write before any request: expect(0) fails the test if
// the DELETE fires. An api-key file profile keeps client resolution off the network.
#[tokio::test]
async fn read_only_blocks_connection_remove_before_writing() {
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
    let cmd = connectors::ConnectorsCmd::Connections {
        cmd: connectors::ConnectionsCmd::Remove {
            connection_id: "c1".into(),
        },
    };
    let err = connectors::run(cmd, &g).await.unwrap_err();
    assert!(
        err.to_string().contains("read-only"),
        "expected a read-only error, got: {err}"
    );

    std::env::remove_var("NESTR_HOME");
}
