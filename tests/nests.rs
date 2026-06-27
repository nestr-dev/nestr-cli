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
