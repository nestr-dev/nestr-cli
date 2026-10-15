use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::recurrence;
use nestr_cli::render;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const INSTANT: i64 = 1_790_600_400_000;

fn page() -> serde_json::Value {
    serde_json::json!({
        "seriesId": "rXPq7WmK4ZtLd3nGf",
        "series": {"_id": "rXPq7WmK4ZtLd3nGf", "title": "Weekly sync", "allDay": false,
                   "start": "2026-09-21T13:00:00.000Z", "due": "2026-09-21T14:00:00.000Z"},
        "allDay": false,
        "occurrences": [
            {"instant": 1_789_995_600_000_i64, "occurrenceStart": "2026-09-21T13:00:00.000Z",
             "start": "2026-09-22T15:00:00.000Z", "allDay": false, "title": "Weekly sync",
             "completed": false, "virtual": false, "nestId": "q7RtY2uV8wXz3AbCd",
             "virtualId": null, "excluded": false, "altered": true},
            {"instant": INSTANT, "occurrenceStart": "2026-09-28T13:00:00.000Z",
             "start": "2026-09-28T13:00:00.000Z", "allDay": false, "title": "Weekly sync",
             "completed": false, "virtual": true, "nestId": null,
             "virtualId": "dZ6f99iD87bzDEYAv", "excluded": false, "altered": false},
            {"instant": 1_791_205_200_000_i64, "occurrenceStart": "2026-10-05T13:00:00.000Z",
             "start": "2026-10-05T13:00:00.000Z", "allDay": false, "title": "Weekly sync",
             "completed": false, "virtual": true, "nestId": null,
             "virtualId": "rBv4gdoPJw66YMBCi", "excluded": true, "altered": false}
        ],
        "hasMore": true,
        "truncated": false,
        "nextCursor": "2026-10-05T13:00:00.000Z",
        "nextCursorMs": 1_791_205_200_000_i64
    })
}

#[tokio::test]
async fn set_patches_the_rule() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1/recurrence"))
        .and(body_json(
            serde_json::json!({"rrule": "FREQ=WEEKLY;BYDAY=MO"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "data": {"rrule": "FREQ=WEEKLY;BYDAY=MO"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::set_recurrence(&client, "n1", Some("FREQ=WEEKLY;BYDAY=MO"))
        .await
        .unwrap();
    assert_eq!(data["rrule"], "FREQ=WEEKLY;BYDAY=MO");
}

#[tokio::test]
async fn remove_sends_a_null_rule() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1/recurrence"))
        .and(body_json(serde_json::json!({"rrule": null})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "data": {"removed": true}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::set_recurrence(&client, "n1", None)
        .await
        .unwrap();
    assert_eq!(data["removed"], true);
}

#[tokio::test]
async fn set_without_a_date_surfaces_the_servers_422() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1/recurrence"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "status": "error", "message": "Set a start or due date before making this repeat"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = recurrence::set_recurrence(&client, "n1", Some("FREQ=DAILY"))
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "validation error: Set a start or due date before making this repeat"
    );
}

#[tokio::test]
async fn occurrences_passes_paging_and_unwraps_the_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/recurrence"))
        .and(query_param("direction", "past"))
        .and(query_param("cursor", "1791205200000"))
        .and(query_param("limit", "3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "success", "data": page()})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::list_occurrences(
        &client,
        "n1",
        &[
            ("direction", "past"),
            ("cursor", "1791205200000"),
            ("limit", "3"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(data["seriesId"], "rXPq7WmK4ZtLd3nGf");
    assert_eq!(data["occurrences"][1]["instant"], INSTANT);
}

#[tokio::test]
async fn skip_deletes_the_instant_without_a_scope() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .and(query_param_is_missing("scope"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "data": {"excluded": true, "seriesId": "s1", "instant": INSTANT}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::skip_occurrence(&client, "n1", INSTANT, false)
        .await
        .unwrap();
    assert_eq!(data["excluded"], true);
}

#[tokio::test]
async fn skip_following_sends_the_following_scope() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .and(query_param("scope", "following"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "data": {"deleted": true, "seriesId": "s1", "instant": INSTANT, "restoreId": "r9"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::skip_occurrence(&client, "n1", INSTANT, true)
        .await
        .unwrap();
    assert_eq!(data["restoreId"], "r9");
}

#[tokio::test]
async fn skip_refused_for_rights_surfaces_the_403() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "status": "error", "message": "You may not delete this nest"
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = recurrence::skip_occurrence(&client, "n1", INSTANT, true)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("You may not delete this nest"),
        "got: {err}"
    );
}

#[tokio::test]
async fn edit_patches_the_instant_and_unwraps_a_bare_nest() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .and(body_json(serde_json::json!({"title": "Sync (moved)"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "_id": "occ1", "title": "Sync (moved)", "data": {"k": "v"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let body = serde_json::json!({"title": "Sync (moved)"});
    let nest = recurrence::edit_occurrence(&client, "n1", INSTANT, &body)
        .await
        .unwrap();
    assert_eq!(nest["_id"], "occ1");
    assert_eq!(nest["data"]["k"], "v");
}

#[tokio::test]
async fn edit_with_no_fields_materializes_and_unwraps_the_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .and(body_json(serde_json::json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "data": {"_id": "occ1", "title": "Weekly sync"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let nest = recurrence::edit_occurrence(&client, "n1", INSTANT, &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(nest["_id"], "occ1");
}

#[tokio::test]
async fn delete_series_deletes_the_recurrence() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/n1/recurrence"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "data": {"deleted": true, "seriesId": "s1", "restoreId": "s1"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = recurrence::delete_series(&client, "n1").await.unwrap();
    assert_eq!(data["restoreId"], "s1");
}

#[test]
fn parse_instant_takes_epoch_milliseconds() {
    assert_eq!(
        recurrence::parse_instant(" 1790600400000 ").unwrap(),
        INSTANT
    );
}

#[test]
fn parse_instant_refuses_a_date_and_points_at_the_listing() {
    let err = recurrence::parse_instant("2026-09-28T13:00:00Z").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("epoch milliseconds"), "got: {msg}");
    assert!(msg.contains("nests recurrence occurrences"), "got: {msg}");
}

#[test]
fn skip_message_for_a_virtual_occurrence_says_the_series_continues() {
    let msg = recurrence::skip_message(&serde_json::json!({
        "excluded": true, "seriesId": "s1", "instant": INSTANT
    }));
    assert_eq!(
        msg,
        "Skipped the occurrence at 1790600400000. The series continues; the instant stays listed as skipped."
    );
}

#[test]
fn skip_message_names_the_nest_to_restore_when_one_was_deleted() {
    let msg = recurrence::skip_message(&serde_json::json!({
        "excluded": true, "seriesId": "s1", "instant": INSTANT, "restoreId": "occ1"
    }));
    assert!(msg.contains("occ1"), "got: {msg}");
    assert!(msg.contains("restore"), "got: {msg}");
}

#[test]
fn skip_message_for_a_cut_says_history_is_kept() {
    let msg = recurrence::skip_message(&serde_json::json!({
        "deleted": true, "seriesId": "s1", "instant": INSTANT, "restoreId": "r9"
    }));
    assert!(
        msg.starts_with("Deleted the occurrence at 1790600400000 and every later one."),
        "got: {msg}"
    );
    assert!(msg.contains("r9"), "got: {msg}");
}

#[test]
fn skip_message_for_a_cut_at_the_first_occurrence_says_the_series_is_gone() {
    let msg = recurrence::skip_message(&serde_json::json!({
        "deleted": true, "seriesId": "s1", "instant": INSTANT, "restoreId": "s1"
    }));
    assert!(msg.contains("whole series was deleted"), "got: {msg}");
}

#[test]
fn occurrences_table_spells_out_each_state() {
    let out = render::occurrences_text(&page());
    assert!(out.contains("Weekly sync"), "got:\n{out}");
    assert!(out.contains("1790600400000"), "got:\n{out}");
    assert!(out.contains("nest, altered"), "got:\n{out}");
    assert!(out.contains("virtual"), "got:\n{out}");
    assert!(out.contains("skipped"), "got:\n{out}");
    assert!(out.contains("--cursor 1791205200000"), "got:\n{out}");
}

#[test]
fn occurrences_text_for_a_nest_without_a_rule() {
    let out = render::occurrences_text(&serde_json::json!({
        "seriesId": null, "series": null, "allDay": false, "occurrences": [],
        "hasMore": false, "truncated": false, "nextCursor": null, "nextCursorMs": null
    }));
    assert!(out.contains("no recurrence rule"), "got:\n{out}");
}

#[test]
fn occurrences_text_says_when_the_walk_was_truncated() {
    let mut p = page();
    p["truncated"] = serde_json::json!(true);
    let out = render::occurrences_text(&p);
    assert!(!out.contains("--cursor"), "got:\n{out}");
    assert!(out.contains("stopped walking"), "got:\n{out}");
}

// A delete under --read-only stops at the gate and fires no API call.
#[tokio::test]
async fn read_only_blocks_skip_before_writing() {
    use nestr_cli::commands::nests;
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
    let cmd = nests::NestsCmd::Recurrence {
        cmd: recurrence::RecurrenceCmd::Skip {
            id: "x".into(),
            instant: INSTANT.to_string(),
            following: true,
        },
    };
    let err = nests::run(cmd, &g).await.unwrap_err();
    assert!(err.to_string().contains("read-only"), "got: {err}");

    std::env::remove_var("NESTR_HOME");
}

const NOT_APPLIED: &str =
    "The schedule was not applied: this item repeats too many times to switch to all-day dates.";

// A schedule write can save and still refuse the dates: 200 with a warning, which
// the client collects (and prints to stderr) while the nest still unwraps.
#[tokio::test]
async fn schedule_write_warning_on_update_is_collected() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "data": {"_id": "n1", "title": "Sync"}, "warnings": [NOT_APPLIED]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut body = serde_json::json!({});
    nestr_cli::commands::nests::apply_schedule(&mut body, None, Some(true), false);
    let data = nestr_cli::commands::nests::update_nest(&client, "n1", &body)
        .await
        .unwrap();
    assert_eq!(data["_id"], "n1");
    assert_eq!(
        client.notices()[0]["warnings"],
        serde_json::json!([NOT_APPLIED])
    );
}

#[tokio::test]
async fn schedule_write_warning_on_edit_is_collected() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/nests/n1/recurrence/{INSTANT}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success", "data": {"_id": "occ1"}, "warnings": [NOT_APPLIED]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let nest =
        recurrence::edit_occurrence(&client, "n1", INSTANT, &serde_json::json!({"allDay": true}))
            .await
            .unwrap();
    assert_eq!(nest["_id"], "occ1");
    assert_eq!(client.notices().len(), 1);
}
