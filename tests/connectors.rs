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
    let data = connectors::fetch_templates(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["id"], "xero");
    let notices = client.notices();
    assert_eq!(
        notices[0]["hints"][0]["label"],
        "Register one with templateId."
    );
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
    let data = connectors::register_connector(&client, "ws1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "k2");
    assert_eq!(
        client.notices()[0]["hints"][0]["label"],
        "Bind it to a role next."
    );
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

// --- what a person sees: shared accounts and held-back connectors -------------

use nestr_cli::render;

#[test]
fn catalog_says_whether_a_connector_allows_a_shared_account() {
    let out = render::connectors_text(&json!([
        {"_id":"c1","name":"Gmail","type":"mcp","authStrategy":"oauth2","enabled":true,
         "exposure":{"userAgent":false,"domainGated":true,"allowShared":false}},
        {"_id":"c2","name":"Ledger","type":"api","authStrategy":"secret","enabled":true,
         "exposure":{"userAgent":false,"domainGated":true,"allowShared":true}}
    ]));
    assert!(out.contains("SHARED"), "{out}");
    let gmail = out.lines().find(|l| l.contains("Gmail")).unwrap();
    let ledger = out.lines().find(|l| l.contains("Ledger")).unwrap();
    assert!(gmail.contains("no"), "{gmail}");
    assert!(ledger.contains("yes"), "{ledger}");
}

#[test]
fn shared_reach_names_who_a_shared_account_would_reach() {
    let s = render::shared_reach_sentence(&json!({
        "boundary":"workspace","people":12,"agents":1,"growsWithMembership":true
    }))
    .unwrap();
    assert!(s.contains("everyone in the workspace"), "{s}");
    assert!(s.contains("12 people and 1 agent"), "{s}");
    assert!(s.contains("anyone who joins later"), "{s}");
    assert!(s.contains("personal account"), "{s}");

    let role = render::shared_reach_sentence(&json!({
        "boundary":"role","people":1,"agents":0,"growsWithMembership":false
    }))
    .unwrap();
    assert!(role.contains("everyone in the role"), "{role}");
    assert!(role.contains("1 person and 0 agents"), "{role}");
    assert!(!role.contains("joins later"), "{role}");
}

#[test]
fn shared_reach_says_nothing_when_sharing_does_not_apply() {
    assert!(render::shared_reach_sentence(&serde_json::Value::Null).is_none());
}

fn held_connection() -> serde_json::Value {
    json!({
        "connectionId":"conn1","connectorName":"Gmail",
        "owner":{"type":"role-domain","id":"d1"},"status":"active","connectedCount":1,
        "authorizations":[{"scope":"personal","connected":true}],
        "pendingInstructionApprovals":[{
            "nestId":"r1","nestTitle":"Finance","connectorName":"Gmail",
            "askedAt":"2026-11-08T09:00:00.000Z","commentPath":"/n/r1?unblock=1","canApprove":true
        }],
        "pendingInstructionApprovalsMore":2
    })
}

#[test]
fn connections_list_marks_a_held_connector_and_says_why() {
    let out = render::connections_text(&json!([held_connection()]), "https://app.example.test/");
    assert!(out.contains("HELD"), "{out}");
    // Not broken: the credential is fine, and the text says who releases it.
    assert!(out.contains("held back on scheduled runs"), "{out}");
    assert!(out.contains("credential is fine"), "{out}");
    assert!(out.contains("Manual runs still use it"), "{out}");
    assert!(
        out.contains("https://app.example.test/n/r1?unblock=1"),
        "{out}"
    );
    assert!(out.contains("Finance"), "{out}");
    assert!(out.contains("you can approve"), "{out}");
    assert!(out.contains("2 more"), "{out}");
}

#[test]
fn connections_list_without_held_connectors_adds_no_notes() {
    let out = render::connections_text(
        &json!([{
            "connectionId":"conn2","connectorName":"Drive","owner":{"type":"workspace","id":"ws"},
            "status":"active","connectedCount":1,"pendingInstructionApprovals":[]
        }]),
        "https://app.example.test",
    );
    assert!(!out.contains("held back"), "{out}");
}

#[test]
fn connections_list_explains_a_shared_account_the_connector_no_longer_allows() {
    let out = render::connections_text(
        &json!([{
            "connectionId":"conn3","connectorName":"CRM","owner":{"type":"workspace","id":"ws"},
            "status":"active","connectedCount":0,
            "authorizations":[{"scope":"shared","connected":false,"sharedNotAllowed":true}]
        }]),
        "https://app.example.test",
    );
    assert!(out.contains("no longer allows shared accounts"), "{out}");
}

#[test]
fn agent_reach_marks_a_connector_held_for_instruction_approval() {
    let reach = json!([
        {"connectionId":"conn1","connectorName":"Gmail","source":"role-domain","available":true,
         "reason":null,"roleTitle":"Finance","domainTitle":"Mailbox","scope":"personal"},
        {"connectionId":"conn2","connectorName":"Drive","source":"workspace","available":true,
         "reason":null,"scope":"shared"}
    ]);
    let out = render::agent_reach_text(&reach, Some(&json!([held_connection()])));
    let gmail = out.lines().find(|l| l.contains("Gmail")).unwrap();
    let drive = out.lines().find(|l| l.contains("Drive")).unwrap();
    assert!(gmail.contains("held"), "{gmail}");
    assert!(!drive.contains("held"), "{drive}");
    assert!(drive.contains("shared"), "{drive}");
    assert!(out.contains("nestr connectors connections list"), "{out}");
}

#[test]
fn agent_reach_without_the_connections_listing_still_renders() {
    let reach = json!([{"connectionId":"conn1","connectorName":"Gmail","available":true}]);
    let out = render::agent_reach_text(&reach, None);
    assert!(out.contains("Gmail"), "{out}");
}

/// Run the real binary against the mock server with an api-key profile written into
/// its own temp dir, so it doesn't race the test here that sets NESTR_HOME.
fn run_cli(host: &str, args: &[&str]) -> std::process::Output {
    use nestr_cli::config::{config_dir_in, AuthKind, CredentialStorage, Profile};
    let tmp = tempfile::tempdir().unwrap();
    let profile = Profile {
        auth: AuthKind::ApiKey,
        credential_storage: CredentialStorage::File,
        host: host.into(),
        workspace_id: "ws".into(),
        api_key: Some("k".into()),
        oauth_client_id: None,
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: None,
        client_instance_id: None,
        default_output_format: None,
    };
    let profiles = config_dir_in(Some(tmp.path().into())).join("profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    std::fs::write(
        profiles.join("t.toml"),
        toml::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();
    std::process::Command::new(env!("CARGO_BIN_EXE_nestr"))
        .args(["--profile", "t", "--yes"])
        .args(args)
        .env("NESTR_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .env_remove("NESTR_API_KEY")
        .env_remove("NESTR_PROFILE")
        .env_remove("NESTR_HOST")
        .env_remove("NESTR_READ_ONLY")
        .env_remove("NESTR_AGENT_MODE")
        .output()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_link_keeps_stdout_the_url_and_warns_about_a_shared_account_on_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/workspaces/ws/connections/c1/connect-link"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"connectionId":"c1","url":"https://app.example.test/n/ws?connect=c1",
                    "reach":{"boundary":"role","people":3,"agents":1,"growsWithMembership":true}}
        })))
        .mount(&server)
        .await;
    let out = run_cli(&server.uri(), &["connectors", "connections", "link", "c1"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {stderr}");
    assert_eq!(stdout.trim(), "https://app.example.test/n/ws?connect=c1");
    assert!(stderr.contains("3 people and 1 agent"), "stderr: {stderr}");
}

#[test]
fn connection_notes_strip_terminal_escapes_from_the_id() {
    let mut c = held_connection();
    c["connectionId"] = json!("conn\u{1b}[31m1");
    let out = render::connections_text(&json!([c]), "https://app.example.test");
    assert!(!out.contains('\u{1b}'), "{out:?}");
}
