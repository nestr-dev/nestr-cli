use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::agents::{self, AgentMode};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_unwraps_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/agents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":[{"_id":"a1","profile":{"fullName":"Collab"},"bot":true,"assistant":false}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::fetch_list(&client, "ws1").await.unwrap();
    assert_eq!(data[0]["_id"], "a1");
}

#[tokio::test]
async fn get_unwraps_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/agents/a1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"a1","profile":{"fullName":"Collab"}}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::fetch_get(&client, "ws1", "a1").await.unwrap();
    assert_eq!(data["profile"]["fullName"], "Collab");
}

#[test]
fn create_body_omits_unset_fields() {
    let body = agents::create_body("Collab".into(), None, false, None);
    assert_eq!(body, json!({"name":"Collab"}));
}

#[test]
fn create_body_assistant_sends_role_assignable_false() {
    let body = agents::create_body(
        "Helper".into(),
        Some("Careful with money".into()),
        true,
        Some(json!({"tokenTtlSeconds":300})),
    );
    assert_eq!(
        body,
        json!({
            "name":"Helper",
            "description":"Careful with money",
            "roleAssignable":false,
            "agentConfig":{"tokenTtlSeconds":300}
        })
    );
}

#[test]
fn parse_json_object_rejects_non_objects() {
    assert!(agents::parse_json_object("--config", "{\"a\":1}").is_ok());
    let err = agents::parse_json_object("--config", "[1]").unwrap_err();
    assert!(err.to_string().contains("--config"));
    assert!(agents::parse_json_object("--config", "not json").is_err());
}

#[tokio::test]
async fn create_posts_body_and_unwraps() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/agents"))
        .and(body_json(json!({"name":"Collab","roleAssignable":false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"a9","profile":{"fullName":"Collab"},"bot":true}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = agents::create_body("Collab".into(), None, true, None);
    let data = agents::create_agent(&client, "ws1", &body).await.unwrap();
    assert_eq!(data["_id"], "a9");
}

#[tokio::test]
async fn run_sends_nest_and_message_and_returns_watch_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/agents/a1/run"))
        .and(body_json(json!({"nestId":"n1","message":"draft the report"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"agentUserId":"a1","nestId":"n1","dispatched":true,"watchUrl":"https://app.test/n/n1"},
            "hints":[{"type":"watch","url":"https://app.test/n/n1"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::run_agent(&client, "ws1", "a1", "n1", Some("draft the report"))
        .await
        .unwrap();
    assert_eq!(data["dispatched"], true);
    assert_eq!(data["watchUrl"], "https://app.test/n/n1");
}

#[tokio::test]
async fn run_without_message_sends_only_nest() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/agents/a1/run"))
        .and(body_json(json!({"nestId":"n1"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"agentUserId":"a1","nestId":"n1","dispatched":true}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::run_agent(&client, "ws1", "a1", "n1", None)
        .await
        .unwrap();
    assert_eq!(data["nestId"], "n1");
}

#[tokio::test]
async fn reach_unwraps_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/agents/a1/connectors"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":[{"connectionId":"c1","connectorName":"Xero","source":"role-domain",
                     "available":false,"reason":"no-credential"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::fetch_reach(&client, "ws1", "a1").await.unwrap();
    assert_eq!(data[0]["reason"], "no-credential");
}

#[tokio::test]
async fn mode_get_hits_nest_scoped_route() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/agents/a1/mode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"agentUserId":"a1","nestId":"n1","mode":"autonomous","recheck":"FREQ=DAILY",
                    "start":"2026-10-15T09:00:00.000Z","tzid":"Europe/Amsterdam",
                    "nextRunAt":"2026-10-16T07:00:00.000Z","source":"inherited"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = agents::fetch_mode(&client, "n1", "a1").await.unwrap();
    assert_eq!(data["source"], "inherited");
    assert_eq!(data["recheck"], "FREQ=DAILY");
}

#[test]
fn mode_body_reactive_sends_mode_only() {
    let body = agents::mode_body(Some(AgentMode::Reactive), None, None, None).unwrap();
    assert_eq!(body, json!({"mode":"reactive"}));
}

#[test]
fn mode_body_reactive_rejects_a_cadence() {
    assert!(agents::mode_body(
        Some(AgentMode::Reactive),
        Some("FREQ=DAILY".into()),
        None,
        None
    )
    .is_err());
}

#[test]
fn mode_body_autonomous_requires_recheck() {
    assert!(agents::mode_body(Some(AgentMode::Autonomous), None, None, None).is_err());
}

#[test]
fn mode_body_autonomous_carries_cadence_start_and_tz() {
    let body = agents::mode_body(
        Some(AgentMode::Autonomous),
        Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".into()),
        Some("2026-10-19T09:00:00".into()),
        Some("Europe/Amsterdam".into()),
    )
    .unwrap();
    assert_eq!(
        body,
        json!({
            "mode":"autonomous",
            "recheck":"FREQ=WEEKLY;BYDAY=MO,WE,FR",
            "start":"2026-10-19T09:00:00",
            "tzid":"Europe/Amsterdam"
        })
    );
}

#[test]
fn mode_body_clear_sends_null_mode() {
    let body = agents::mode_body(None, None, None, None).unwrap();
    assert_eq!(body, json!({"mode":null}));
}

#[tokio::test]
async fn set_mode_puts_body_and_returns_hints() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/nests/n1/agents/a1/mode"))
        .and(body_json(
            json!({"mode":"autonomous","recheck":"continuous"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success",
            "data":{"agentUserId":"a1","nestId":"n1","mode":"autonomous","recheck":"continuous",
                    "start":null,"tzid":null,"nextRunAt":null},
            "hints":[{"type":"agent_prefer_scheduled_task","severity":"info",
                      "label":"Prefer a repeating task for a recurring job."}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = agents::mode_body(
        Some(AgentMode::Autonomous),
        Some("continuous".into()),
        None,
        None,
    )
    .unwrap();
    let (data, hints) = agents::set_mode(&client, "n1", "a1", &body).await.unwrap();
    assert_eq!(data["recheck"], "continuous");
    assert_eq!(hints, vec!["Prefer a repeating task for a recurring job."]);
}

#[tokio::test]
async fn clear_mode_returns_cleared() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/nests/n1/agents/a1/mode"))
        .and(body_json(json!({"mode":null})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"agentUserId":"a1","nestId":"n1","cleared":true}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = agents::mode_body(None, None, None, None).unwrap();
    let (data, hints) = agents::set_mode(&client, "n1", "a1", &body).await.unwrap();
    assert_eq!(data["cleared"], true);
    assert!(hints.is_empty());
}

#[tokio::test]
async fn api_key_run_surfaces_server_refusal() {
    // A workspace API key carries no user scope, and the run route refuses it.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspaces/ws1/agents/a1/run"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "status":"error",
            "message":"A user-scoped token is required to run an agent, so the run has a requester"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    assert!(agents::run_agent(&client, "ws1", "a1", "n1", None)
        .await
        .is_err());
}

// --read-only must stop a write before any request: expect(0) fails the test if
// the PUT fires. An api-key file profile keeps client resolution off the network.
#[tokio::test]
async fn read_only_blocks_mode_set_before_writing() {
    use nestr_cli::commands::GlobalArgs;
    use nestr_cli::config::{save_profile, AuthKind, CredentialStorage, Profile};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
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
    let cmd = agents::AgentsCmd::Mode {
        cmd: agents::ModeCmd::Clear {
            agent_id: "a1".into(),
            nest: "n1".into(),
        },
    };
    let err = agents::run(cmd, &g).await.unwrap_err();
    assert!(
        err.to_string().contains("read-only"),
        "expected a read-only error, got: {err}"
    );

    std::env::remove_var("NESTR_HOME");
}
