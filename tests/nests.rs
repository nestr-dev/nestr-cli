use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::nests;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_single_nest_unwraps_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n1","title":"Root"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::fetch_get(&client, "n1", &[]).await.unwrap();
    assert_eq!(data["_id"], "n1");
}

#[tokio::test]
async fn get_multiple_ids_comma_joined() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/a,b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"a"},{"_id":"b"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::fetch_get(&client, "a,b", &[]).await.unwrap();
    assert!(data.is_array());
}

#[tokio::test]
async fn children_unwraps_array_with_meta() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/children"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"c1","title":"Child"}],
            "meta":{"page":1,"total_pages":1,"total":1}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, meta) = nests::fetch_children(&client, "n1", &[]).await.unwrap();
    assert_eq!(data[0]["_id"], "c1");
    assert_eq!(meta.unwrap()["total"], 1);
}

#[tokio::test]
async fn create_posts_body_and_returns_nest() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"new1","title":"Task"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title":"Task","parentId":"p1"});
    let data = nests::create_nest(&client, &body).await.unwrap();
    assert_eq!(data["_id"], "new1");
}

#[tokio::test]
async fn resolve_assignees_passes_plain_ids_through_without_calling_me() {
    // No `me` token → the helper must not touch /users/me; ids pass straight through.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let out = nests::resolve_assignees(&client, &["u1".into(), "u2".into()])
        .await
        .unwrap();
    assert_eq!(out, vec!["u1".to_string(), "u2".to_string()]);
}

#[tokio::test]
async fn resolve_assignees_expands_me_to_current_user_id() {
    // A `me` token is replaced by the authenticated user's id; other ids are kept.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"meId","username":"a@b.c"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let out = nests::resolve_assignees(&client, &["u1".into(), "me".into()])
        .await
        .unwrap();
    assert_eq!(out, vec!["u1".to_string(), "meId".to_string()]);
}

#[test]
fn create_body_maps_fields_and_assigns_users() {
    let body = nests::create_body(
        "bsv-eu-dr-01".into(),
        Some("role1".into()),
        None,
        Some("details".into()),
        &["project".into()],
        Some("2026-07-01".into()),
        &["meId".into()],
        &serde_json::Map::new(),
    );
    assert_eq!(body["title"], "bsv-eu-dr-01");
    assert_eq!(body["parentId"], "role1");
    assert_eq!(body["description"], "details");
    assert_eq!(body["labels"], serde_json::json!(["project"]));
    assert_eq!(body["due"], "2026-07-01");
    // The whole point: an assigned project carries its user in `users`.
    assert_eq!(body["users"], serde_json::json!(["meId"]));
    // Purpose was None → not sent.
    assert!(body.get("purpose").is_none());
}

#[test]
fn create_body_omits_users_and_optionals_when_absent() {
    let body = nests::create_body(
        "Loose todo".into(),
        None,
        None,
        None,
        &[],
        None,
        &[],
        &serde_json::Map::new(),
    );
    assert_eq!(body["title"], "Loose todo");
    // No assignee → no `users` key at all (don't send an empty array).
    assert!(body.get("users").is_none());
    assert!(body.get("parentId").is_none());
    assert!(body.get("labels").is_none());
}

#[test]
fn update_body_sets_users_only_when_given() {
    let body = nests::update_body(
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
        &["u1".into(), "u2".into()],
        &serde_json::Map::new(),
    );
    assert_eq!(body["users"], serde_json::json!(["u1", "u2"]));
    assert_eq!(
        body.as_object().unwrap().len(),
        1,
        "only `users` should be set"
    );

    let empty = nests::update_body(
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
        &[],
        &serde_json::Map::new(),
    );
    assert!(empty.as_object().unwrap().is_empty());
}

#[tokio::test]
async fn bulk_reorder_sends_bare_array_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/workspaces/ws1/reorder"))
        .and(wiremock::matchers::body_json(serde_json::json!([
            "a", "b", "c"
        ])))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[{"_id":"a"},{"_id":"b"},{"_id":"c"}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::bulk_reorder(&client, "ws1", &["a".into(), "b".into(), "c".into()])
        .await
        .unwrap();
    assert!(data.is_array());
}

#[tokio::test]
async fn label_add_rejects_a_second_prime_before_writing() {
    let server = MockServer::start().await;
    // The nest is already a role; adding `project` must be refused after the GET and
    // before any add_label PATCH fires.
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n1","title":"Facilitator","labels":["role"]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1/add_label/project"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = nests::ensure_prime_compatible(&client, "n1", "project")
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("already a 'role'"),
        "expected a one-prime error, got: {err}"
    );
}

#[tokio::test]
async fn label_add_allows_userstory_on_an_existing_project() {
    // Promoting a project to a Scrum story. `userstory` implies `project` in the
    // data model (nestr-web `packages/nestr_scrum/labels.js`), so this is not a
    // second identity and the guard must let it through.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n4","title":"Ship the thing","labels":["project"]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    nests::ensure_prime_compatible(&client, "n4", "userstory")
        .await
        .expect("userstory implies project, so adding it to a project is allowed");
}

#[tokio::test]
async fn label_add_still_rejects_sprint_on_a_project() {
    // Only `userstory` carries an implication — a sprint is a separate container.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n5","title":"Ship the thing","labels":["project"]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = nests::ensure_prime_compatible(&client, "n5", "sprint")
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("already a 'project'"),
        "expected a one-prime error, got: {err}"
    );
}

#[tokio::test]
async fn label_add_allows_a_prime_when_the_nest_has_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n2","title":"Loose todo","labels":["now"]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    nests::ensure_prime_compatible(&client, "n2", "project")
        .await
        .expect("adding a prime to a nest with no prime should be allowed");
}

#[tokio::test]
async fn label_add_of_a_non_prime_skips_the_fetch() {
    // A non-prime label never conflicts, so the guard must not even GET the nest.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n3"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    nests::ensure_prime_compatible(&client, "n3", "urgent")
        .await
        .expect("adding a non-prime label should be allowed without a fetch");
}

#[tokio::test]
async fn delete_hits_delete_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/x"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"message":"deleted","nestId":"x"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::delete_nest(&client, "x").await.unwrap();
    assert_eq!(data["nestId"], "x");
}

// Proves the spec §11 / §8 read-only guarantee end-to-end: a write subcommand
// run under `--read-only` errors via the gate and fires NO write API call. Uses an
// api-key file profile so client resolution touches no network at all.
#[tokio::test]
async fn read_only_blocks_delete_before_writing() {
    use nestr_cli::commands::GlobalArgs;
    use nestr_cli::config::{save_profile, AuthKind, CredentialStorage, Profile};

    let server = MockServer::start().await;
    // If the gate failed, the DELETE would fire; expect(0) asserts it never does.
    Mock::given(method("DELETE"))
        .and(path("/api/nests/x"))
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
    let err = nests::run(nests::NestsCmd::Delete { id: "x".into() }, &g)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("read-only"),
        "expected a read-only error, got: {err}"
    );

    std::env::remove_var("NESTR_HOME");
}

// --- --field: app/namespaced field writes (issue #35) ---

#[test]
fn parse_field_args_types_json_values_and_keeps_strings() {
    let fields = nests::parse_field_args(&[
        "sprint.capacity=20".into(),
        "sprint.status=active".into(),
        r#"sprint.term={"from":"2026-08-01","to":"2026-08-14"}"#.into(),
        "userstory.points=3.5".into(),
        "custom.flag=true".into(),
        r#"custom.forced_string="true""#.into(),
    ])
    .unwrap();
    assert_eq!(fields["sprint.capacity"], serde_json::json!(20));
    assert_eq!(fields["sprint.status"], serde_json::json!("active"));
    assert_eq!(
        fields["sprint.term"],
        serde_json::json!({"from":"2026-08-01","to":"2026-08-14"})
    );
    assert_eq!(fields["userstory.points"], serde_json::json!(3.5));
    assert_eq!(fields["custom.flag"], serde_json::json!(true));
    // Quoting the value as JSON forces a string even when it would parse as
    // another type.
    assert_eq!(fields["custom.forced_string"], serde_json::json!("true"));
}

#[test]
fn parse_field_args_keeps_equals_signs_in_values() {
    let fields = nests::parse_field_args(&["custom.formula=a=b".into()]).unwrap();
    assert_eq!(fields["custom.formula"], serde_json::json!("a=b"));
}

#[test]
fn parse_field_args_rejects_malformed_pairs() {
    let err = nests::parse_field_args(&["sprint.capacity".into()]).unwrap_err();
    assert!(
        err.to_string().contains("KEY=VALUE"),
        "expected a KEY=VALUE hint, got: {err}"
    );
    let err = nests::parse_field_args(&["=20".into()]).unwrap_err();
    assert!(
        err.to_string().contains("KEY=VALUE"),
        "expected a KEY=VALUE hint, got: {err}"
    );
}

#[test]
fn create_body_includes_fields_object() {
    let fields = nests::parse_field_args(&[
        "sprint.capacity=20".into(),
        r#"sprint.term={"from":"2026-08-01","to":"2026-08-14"}"#.into(),
    ])
    .unwrap();
    let body = nests::create_body(
        "Sprint 12".into(),
        Some("circle1".into()),
        None,
        None,
        &["sprint".into()],
        None,
        &[],
        &fields,
    );
    assert_eq!(
        body["fields"],
        serde_json::json!({
            "sprint.capacity": 20,
            "sprint.term": {"from":"2026-08-01","to":"2026-08-14"}
        })
    );
}

#[test]
fn create_body_omits_fields_when_absent() {
    let body = nests::create_body(
        "Loose todo".into(),
        None,
        None,
        None,
        &[],
        None,
        &[],
        &serde_json::Map::new(),
    );
    assert!(body.get("fields").is_none());
}

#[test]
fn update_body_includes_fields_object() {
    let fields = nests::parse_field_args(&["sprint.status=active".into()]).unwrap();
    let body = nests::update_body(None, None, None, None, None, None, &[], &[], &fields);
    assert_eq!(
        body["fields"],
        serde_json::json!({"sprint.status": "active"})
    );
    assert_eq!(
        body.as_object().unwrap().len(),
        1,
        "only `fields` should be set"
    );
}

#[test]
fn apply_schedule_sends_start_and_all_day() {
    let mut body = serde_json::json!({"title": "Offsite"});
    nests::apply_schedule(
        &mut body,
        Some("2026-10-05".into()),
        Some(true),
        false,
        false,
    );
    assert_eq!(body["start"], "2026-10-05");
    assert_eq!(body["allDay"], true);
    assert_eq!(body["title"], "Offsite");
}

#[test]
fn apply_schedule_sends_all_day_false_to_flip_back_to_instants() {
    let mut body = serde_json::json!({});
    nests::apply_schedule(&mut body, None, Some(false), false, false);
    assert_eq!(body, serde_json::json!({"allDay": false}));
}

#[test]
fn apply_schedule_clear_start_sends_null() {
    let mut body = serde_json::json!({});
    nests::apply_schedule(&mut body, None, None, true, false);
    assert_eq!(body, serde_json::json!({"start": null}));
}

#[test]
fn apply_schedule_clear_due_sends_null() {
    let mut body = serde_json::json!({});
    nests::apply_schedule(&mut body, None, None, false, true);
    assert_eq!(body, serde_json::json!({"due": null}));
}

#[test]
fn apply_schedule_leaves_body_alone_when_nothing_given() {
    let mut body = serde_json::json!({"title": "x"});
    nests::apply_schedule(&mut body, None, None, false, false);
    assert_eq!(body, serde_json::json!({"title": "x"}));
}

#[tokio::test]
async fn update_with_bad_start_surfaces_the_servers_422() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"start": "next tuesday"}),
        ))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "status": "error",
            "message": "start must be an ISO 8601 date-time or a yyyy-mm-dd date"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut body = serde_json::json!({});
    nests::apply_schedule(&mut body, Some("next tuesday".into()), None, false, false);
    let err = nests::update_nest(&client, "n1", &body).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "validation error: start must be an ISO 8601 date-time or a yyyy-mm-dd date"
    );
}

/// Run the real binary against the mock server with an api-key profile. The profile
/// is written straight into the temp dir: setting NESTR_HOME process-wide would race
/// the other tests in this binary that do.
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

/// Run `nestr nests update n1 <args>` and assert the PATCH body is exactly `body`.
async fn assert_update_sends(args: &[&str], body: serde_json::Value) {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/nests/n1"))
        .and(wiremock::matchers::body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n1","title":"T"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let mut argv = vec!["-o", "json", "nests", "update", "n1"];
    argv.extend_from_slice(args);
    let out = run_cli(&server.uri(), &argv);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_clear_due_sends_due_null() {
    assert_update_sends(&["--clear-due"], serde_json::json!({"due": null})).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn update_clear_start_and_clear_due_sends_both_null() {
    assert_update_sends(
        &["--clear-start", "--clear-due"],
        serde_json::json!({"start": null, "due": null}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn update_clear_due_combines_with_start_and_all_day() {
    assert_update_sends(
        &["--start", "2026-10-05", "--all-day", "--clear-due"],
        serde_json::json!({"start": "2026-10-05", "allDay": true, "due": null}),
    )
    .await;
}

#[test]
fn apply_proxy_assign_sends_the_role_id() {
    let mut body = serde_json::json!({"title": "Draft"});
    nests::apply_proxy_assign(&mut body, Some("role-1".into()), false);
    assert_eq!(
        body,
        serde_json::json!({"title": "Draft", "proxyAssign": "role-1"})
    );
}

#[test]
fn apply_proxy_assign_clear_sends_null() {
    let mut body = serde_json::json!({});
    nests::apply_proxy_assign(&mut body, None, true);
    assert_eq!(body, serde_json::json!({"proxyAssign": null}));
}

#[test]
fn apply_proxy_assign_leaves_it_out_when_not_passed() {
    let mut body = serde_json::json!({"title": "Draft"});
    nests::apply_proxy_assign(&mut body, None, false);
    assert_eq!(body, serde_json::json!({"title": "Draft"}));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_proxy_assign_sends_the_role_id() {
    assert_update_sends(
        &["--proxy-assign", "role-1"],
        serde_json::json!({"proxyAssign": "role-1"}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn update_clear_proxy_assign_sends_null() {
    assert_update_sends(
        &["--clear-proxy-assign"],
        serde_json::json!({"proxyAssign": null}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn create_proxy_assign_sends_the_role_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/nests"))
        .and(wiremock::matchers::body_partial_json(
            serde_json::json!({"title": "Draft", "proxyAssign": "role-1"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"n2","title":"Draft","proxyAssign":"role-1"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let out = run_cli(
        &server.uri(),
        &[
            "-o",
            "json",
            "nests",
            "create",
            "--title",
            "Draft",
            "--parent",
            "p1",
            "--proxy-assign",
            "role-1",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {stderr}");
}
