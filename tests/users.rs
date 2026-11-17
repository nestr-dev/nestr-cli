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

// ---- People labels and fields are written on the member route: PATCH
// /workspaces/:ws/users/:id with `labels` (the whole set) and `fields` keyed
// `<label>.<field>`, the shapes nestr-web's own API tests send.

const MEMBER: &str = "/workspaces/ws1/users/u1";
const REGISTER: &str = "https://example.com/r/1";

fn member(labels: &[&str], fields: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "status":"success",
        "data":{"_id":"u1","username":"a@example.com","labels":labels,"fields":fields}
    })
}

async fn mount_read(server: &MockServer, user: &str, reply: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(user))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_no_write(server: &MockServer, user: &str) {
    Mock::given(method("PATCH"))
        .and(path(user))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(server)
        .await;
}

fn fields(pairs: &[&str]) -> serde_json::Map<String, serde_json::Value> {
    let pairs: Vec<String> = pairs.iter().map(|p| p.to_string()).collect();
    nestr_cli::commands::nests::parse_field_args(&pairs).unwrap()
}

#[tokio::test]
async fn label_add_sends_the_whole_set_with_fields_keyed_by_label() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        member(&["skill_coding"], serde_json::json!({})),
    )
    .await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        // The label it already held is kept, not removed.
        .and(body_json(serde_json::json!({
            "labels":["skill_coding","legal_rep"],
            "fields":{"legal_rep.url":REGISTER,"legal_rep.level":3}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(member(
            &["skill_coding", "legal_rep"],
            serde_json::json!({"legal_rep.url":REGISTER,"legal_rep.level":3}),
        )))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::add_user_label(
        &client,
        "ws1",
        "u1",
        "legal_rep",
        &fields(&[&format!("url={REGISTER}"), "level=3"]),
    )
    .await
    .unwrap();
    assert_eq!(
        data,
        serde_json::json!({
            "userId":"u1","label":"legal_rep","changed":true,
            "labels":["skill_coding","legal_rep"],
            "fields":{"legal_rep.url":REGISTER,"legal_rep.level":3}
        })
    );
}

#[tokio::test]
async fn label_add_without_fields_sends_the_set_alone() {
    let server = MockServer::start().await;
    mount_read(&server, MEMBER, member(&[], serde_json::json!({}))).await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        .and(body_json(serde_json::json!({"labels":["skill_coding"]})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(member(&["skill_coding"], serde_json::json!({}))),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::add_user_label(&client, "ws1", "u1", "skill_coding", &fields(&[]))
        .await
        .unwrap();
    assert_eq!(data["changed"], true);
}

#[tokio::test]
async fn label_add_takes_a_qualified_id_and_a_label_dot_field_key() {
    let server = MockServer::start().await;
    mount_read(&server, MEMBER, member(&[], serde_json::json!({}))).await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        // The short id the member payload uses, and the key is not prefixed twice.
        .and(body_json(serde_json::json!({
            "labels":["legal_rep"],"fields":{"legal_rep.url":REGISTER}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(member(
            &["legal_rep"],
            serde_json::json!({"legal_rep.url":REGISTER}),
        )))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::add_user_label(
        &client,
        "ws1",
        "u1",
        "ws1:legal_rep",
        &fields(&[&format!("legal_rep.url={REGISTER}")]),
    )
    .await
    .unwrap();
    assert_eq!(data["label"], "legal_rep");
}

#[tokio::test]
async fn label_add_of_a_held_label_writes_nothing() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        member(
            &["legal_rep"],
            serde_json::json!({"legal_rep.url":REGISTER}),
        ),
    )
    .await;
    mount_no_write(&server, MEMBER).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    // The stored value is left alone, as the old add route left it.
    let data = users::add_user_label(
        &client,
        "ws1",
        "u1",
        "legal_rep",
        &fields(&["url=https://example.com/other"]),
    )
    .await
    .unwrap();
    assert_eq!(data["changed"], false);
    assert_eq!(data["fields"]["legal_rep.url"], REGISTER);
}

#[tokio::test]
async fn label_set_patches_fields_only_and_an_empty_value_clears() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        member(
            &["skill_coding"],
            serde_json::json!({"skill_coding.level":"3","skill_coding.notes":"x"}),
        ),
    )
    .await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        // No `labels` key, so the member's set is left alone; null clears.
        .and(body_json(serde_json::json!({
            "fields":{"skill_coding.level":4,"skill_coding.notes":null}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(member(
            &["skill_coding"],
            serde_json::json!({"skill_coding.level":4}),
        )))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::set_user_label_fields(
        &client,
        "ws1",
        "u1",
        "skill_coding",
        &fields(&["level=4", "notes="]),
    )
    .await
    .unwrap();
    assert_eq!(data["changed"], true);
    assert_eq!(data["fields"], serde_json::json!({"skill_coding.level":4}));
}

#[tokio::test]
async fn label_set_to_the_stored_value_reports_no_change() {
    let server = MockServer::start().await;
    let held = member(
        &["skill_coding"],
        serde_json::json!({"skill_coding.level":"3"}),
    );
    mount_read(&server, MEMBER, held.clone()).await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        .respond_with(ResponseTemplate::new(200).set_body_json(held))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::set_user_label_fields(
        &client,
        "ws1",
        "u1",
        "skill_coding",
        &fields(&["level=\"3\""]),
    )
    .await
    .unwrap();
    assert_eq!(data["changed"], false);
}

#[tokio::test]
async fn label_remove_sends_the_set_without_it_keeping_the_others() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        member(&["legal_rep", "skill_coding"], serde_json::json!({})),
    )
    .await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        .and(body_json(serde_json::json!({"labels":["skill_coding"]})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(member(&["skill_coding"], serde_json::json!({}))),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::remove_user_label(&client, "ws1", "u1", "ws1:legal_rep")
        .await
        .unwrap();
    assert_eq!(data["changed"], true);
    assert_eq!(data["labels"], serde_json::json!(["skill_coding"]));
}

#[tokio::test]
async fn label_remove_of_a_label_not_held_writes_nothing() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        member(&["skill_coding"], serde_json::json!({})),
    )
    .await;
    mount_no_write(&server, MEMBER).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = users::remove_user_label(&client, "ws1", "u1", "legal_rep")
        .await
        .unwrap();
    assert_eq!(data["changed"], false);
}

#[tokio::test]
async fn label_write_on_a_server_that_ignores_member_labels_is_refused() {
    // An older server's member PATCH takes the body, ignores `labels`, and answers
    // with the plain member. That must not print as "added".
    let server = MockServer::start().await;
    mount_read(&server, MEMBER, member(&[], serde_json::json!({}))).await;
    Mock::given(method("PATCH"))
        .and(path(MEMBER))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":{"_id":"u1","username":"a@example.com"}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let e = users::add_user_label(&client, "ws1", "u1", "legal_rep", &fields(&[]))
        .await
        .unwrap_err();
    let msg = e.to_string();
    assert!(msg.contains("2026-11-17"), "{msg}");
    assert!(matches!(e, nestr_cli::error::NestrError::Ignored(_)));
}

#[tokio::test]
async fn label_write_on_a_server_without_user_labels_is_refused_before_writing() {
    let server = MockServer::start().await;
    mount_read(
        &server,
        MEMBER,
        serde_json::json!({"status":"success","data":{"_id":"u1","username":"a@example.com"}}),
    )
    .await;
    mount_no_write(&server, MEMBER).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let e = users::remove_user_label(&client, "ws1", "u1", "legal_rep")
        .await
        .unwrap_err();
    assert!(e.to_string().contains("newer Nestr server"), "{e}");
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
async fn label_errors_from_the_member_patch_keep_code_and_map_status() {
    let server = MockServer::start().await;
    for (status, code) in [
        (400, "label-not-applicable"),
        (403, "not-authorized"),
        (404, "not-member"),
    ] {
        let user = format!("/workspaces/ws1/users/u{status}");
        mount_read(&server, &user, member(&[], serde_json::json!({}))).await;
        Mock::given(method("PATCH"))
            .and(path(user))
            .respond_with(
                ResponseTemplate::new(status).set_body_json(serde_json::json!({
                    "status":"error","error":code,"message":"nope"
                })),
            )
            .mount(&server)
            .await;
    }
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let none = fields(&[]);
    let e = users::add_user_label(&client, "ws1", "u400", "x", &none)
        .await
        .unwrap_err();
    assert!(
        matches!(e, nestr_cli::error::NestrError::Validation(ref m) if m.contains("label-not-applicable"))
    );
    let e = users::add_user_label(&client, "ws1", "u403", "x", &none)
        .await
        .unwrap_err();
    assert!(
        matches!(e, nestr_cli::error::NestrError::Permission(ref m) if m.contains("not-authorized"))
    );
    let e = users::add_user_label(&client, "ws1", "u404", "x", &none)
        .await
        .unwrap_err();
    assert!(matches!(e, nestr_cli::error::NestrError::NotFound(ref m) if m.contains("not-member")));
}

#[test]
fn label_bodies_match_the_member_route_shape() {
    let held = vec!["a".to_string(), "b".to_string()];
    assert_eq!(
        users::remove_label_body(&held, "a"),
        serde_json::json!({"labels":["b"]})
    );
    assert_eq!(
        users::add_label_body(&held, "c", &fields(&["x=1", "c.y=two"])),
        serde_json::json!({"labels":["a","b","c"],"fields":{"c.x":1,"c.y":"two"}})
    );
    assert_eq!(
        users::set_fields_body("c", &fields(&["x="])),
        serde_json::json!({"fields":{"c.x":null}})
    );
    assert_eq!(users::short_label_id("ws1", "ws1:legal_rep"), "legal_rep");
    assert_eq!(users::short_label_id("ws1", "legal_rep"), "legal_rep");
    // Only the colon prefix is the workspace's, as the server shortens it.
    assert_eq!(users::short_label_id("ws1", "ws1_rep"), "ws1_rep");
}

// ---- 0.5.2: `--label` on a server without user labels (production's master ignores
// the param and returns every member) must not read as a filtered list.

#[test]
fn label_filter_is_confirmed_by_a_reply_whose_rows_carry_the_label() {
    // Shaped like the user-labels server: every row has `labels` (short ids).
    let data = serde_json::json!([
        {"_id":"u1","labels":["legal_rep","skill_coding"]},
        {"_id":"u2","labels":["legal_rep"]}
    ]);
    assert!(users::confirm_label_filter(&data, "legal_rep", "ws1").is_ok());
    // The label given by its full id matches the short id on the rows.
    assert!(users::confirm_label_filter(&data, "ws1_legal_rep", "ws1").is_ok());
    // Nobody carries it: nothing to contradict.
    assert!(users::confirm_label_filter(&serde_json::json!([]), "legal_rep", "ws1").is_ok());
}

#[test]
fn label_filter_is_refused_when_the_server_ignored_it() {
    // Shaped like master: no `labels` on any row.
    let master = serde_json::json!([{"_id":"u1","profile":{}}, {"_id":"u2","profile":{}}]);
    let err = users::confirm_label_filter(&master, "legal_rep", "ws1").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ignores --label"), "{msg}");
    assert!(msg.contains("needs a newer Nestr server"), "{msg}");
    // A row without the label means the filter wasn't applied.
    let mixed = serde_json::json!([{"_id":"u1","labels":["legal_rep"]}, {"_id":"u2","labels":[]}]);
    assert!(users::confirm_label_filter(&mixed, "legal_rep", "ws1").is_err());
}
