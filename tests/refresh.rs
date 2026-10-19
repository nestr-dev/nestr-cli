use nestr_cli::api_client::NestrClient;
use nestr_cli::config::{load_profile, CredentialStorage};
use nestr_cli::error::NestrError;
use nestr_cli::oauth::{unix_now_secs, ReactiveRefresh, StoredOAuthTokens};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Tests that point NESTR_HOME at a temp dir hold this so they don't race.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const LOGGED_OUT: &str = "You must be logged in to do this.";
const SINGLE_USER_SCOPE: &str = "This endpoint is only available for single user scoped access";

fn refresh_for(server: &MockServer, profile: &str) -> ReactiveRefresh {
    ReactiveRefresh::new(
        format!("{}/oauth/token", server.uri()),
        "client-1".into(),
        profile.into(),
        CredentialStorage::File,
    )
}

/// The token endpoint rotates the pair, as Nestr does on every refresh.
async fn mount_token_endpoint(server: &MockServer, expected_calls: u64) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "fresh-token",
            "token_type": "bearer",
            "expires_in": 3600,
            "refresh_token": "fresh-refresh"
        })))
        .expect(expected_calls)
        .mount(server)
        .await;
}

async fn mount_status(
    server: &MockServer,
    verb: &str,
    route: &str,
    bearer: &str,
    status: u16,
    msg: &str,
) {
    Mock::given(method(verb))
        .and(path(route))
        .and(header("authorization", format!("Bearer {bearer}").as_str()))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_json(serde_json::json!({"status":"error","message": msg})),
        )
        .mount(server)
        .await;
}

async fn mount_ok(server: &MockServer, route: &str, bearer: &str) {
    Mock::given(method("GET"))
        .and(path(route))
        .and(header("authorization", format!("Bearer {bearer}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"_id":"x"})))
        .mount(server)
        .await;
}

#[tokio::test]
async fn refreshes_and_retries_once_on_401() {
    let _env = ENV_LOCK.lock().await;
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 1).await;
    mount_status(&server, "GET", "/nests/x", "stale", 401, LOGGED_OUT).await;
    mount_ok(&server, "/nests/x", "fresh-token").await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile("p401", &server.uri(), "stale", Some(0));

    let client =
        NestrClient::with_refresh(server.uri(), "stale", Some(refresh_for(&server, "p401")))
            .unwrap();
    let v: serde_json::Value = client.get("/nests/x", &[]).await.unwrap();
    assert_eq!(v["_id"], "x");
    std::env::remove_var("NESTR_HOME");
}

#[tokio::test]
async fn refreshed_pair_is_persisted_even_while_the_stored_token_looks_valid() {
    // A 401 arrives for a token whose local expiry is still in the future (the server
    // revoked it). The server rotates the pair on refresh and deletes the old one, so
    // the new pair has to land on disk or every later process is logged out.
    let _env = ENV_LOCK.lock().await;
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 1).await;
    mount_status(&server, "GET", "/nests/x", "stale", 401, LOGGED_OUT).await;
    mount_ok(&server, "/nests/x", "fresh-token").await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile(
        "pvalid",
        &server.uri(),
        "stale",
        Some(unix_now_secs() + 3000),
    );

    let client =
        NestrClient::with_refresh(server.uri(), "stale", Some(refresh_for(&server, "pvalid")))
            .unwrap();
    client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap();

    let stored = load_profile("pvalid").unwrap().oauth_tokens.unwrap();
    assert_eq!(stored.access_token, "fresh-token");
    assert_eq!(stored.refresh_token.as_deref(), Some("fresh-refresh"));
    std::env::remove_var("NESTR_HOME");
}

#[tokio::test]
async fn adopts_a_pair_another_process_already_refreshed() {
    // Another `nestr` process rotated the pair while this one held the old access
    // token. Refreshing again with our (now revoked) refresh token would fail, so
    // the client must pick up the stored pair instead of calling the token endpoint.
    let _env = ENV_LOCK.lock().await;
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 0).await;
    mount_status(&server, "GET", "/nests/x", "stale", 401, LOGGED_OUT).await;
    mount_ok(&server, "/nests/x", "rotated-elsewhere").await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile(
        "pother",
        &server.uri(),
        "rotated-elsewhere",
        Some(unix_now_secs() + 3000),
    );

    let client =
        NestrClient::with_refresh(server.uri(), "stale", Some(refresh_for(&server, "pother")))
            .unwrap();
    let v: serde_json::Value = client.get("/nests/x", &[]).await.unwrap();
    assert_eq!(v["_id"], "x");
    std::env::remove_var("NESTR_HOME");
}

#[tokio::test]
async fn scope_403_does_not_refresh_and_says_what_the_grant_lacks() {
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 0).await;
    mount_status(
        &server,
        "GET",
        "/users/me/activity",
        "tok",
        403,
        SINGLE_USER_SCOPE,
    )
    .await;

    let client =
        NestrClient::with_refresh(server.uri(), "tok", Some(refresh_for(&server, "unused")))
            .unwrap();
    let err = client
        .get::<serde_json::Value>("/users/me/activity", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Permission(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains("your own user account"), "{msg}");
    assert!(msg.contains("workspace"), "{msg}");
}

#[tokio::test]
async fn other_403_does_not_refresh() {
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 0).await;
    mount_status(
        &server,
        "GET",
        "/nests/x",
        "tok",
        403,
        "You do not have permission to do this.",
    )
    .await;

    let client =
        NestrClient::with_refresh(server.uri(), "tok", Some(refresh_for(&server, "unused")))
            .unwrap();
    let err = client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Permission(_)), "{err:?}");
}

#[tokio::test]
async fn failed_refresh_says_the_session_is_gone() {
    let _env = ENV_LOCK.lock().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "Invalid grant: refresh token is invalid"
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_status(&server, "GET", "/nests/x", "stale", 401, LOGGED_OUT).await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile(
        "pdead",
        &server.uri(),
        "stale",
        Some(unix_now_secs() + 3000),
    );

    let client =
        NestrClient::with_refresh(server.uri(), "stale", Some(refresh_for(&server, "pdead")))
            .unwrap();
    let err = client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Auth(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains("expired or revoked"), "{msg}");
    assert!(msg.contains("nestr auth login"), "{msg}");
    std::env::remove_var("NESTR_HOME");
}

#[tokio::test]
async fn still_401_after_refresh_says_the_session_is_gone() {
    let _env = ENV_LOCK.lock().await;
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 1).await;
    mount_status(&server, "GET", "/nests/x", "stale", 401, LOGGED_OUT).await;
    mount_status(&server, "GET", "/nests/x", "fresh-token", 401, LOGGED_OUT).await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile("pstill", &server.uri(), "stale", Some(0));

    let client =
        NestrClient::with_refresh(server.uri(), "stale", Some(refresh_for(&server, "pstill")))
            .unwrap();
    let err = client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Auth(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains("expired or revoked"), "{msg}");
    assert!(msg.contains("nestr auth login"), "{msg}");
    std::env::remove_var("NESTR_HOME");
}

#[tokio::test]
async fn api_key_401_is_an_auth_error_not_permission_denied() {
    let server = MockServer::start().await;
    mount_status(&server, "GET", "/nests/x", "key", 401, LOGGED_OUT).await;

    let client = NestrClient::new(server.uri(), "key").unwrap();
    let err = client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Auth(_)), "{err:?}");
    assert!(err.to_string().contains("API key"), "{err}");
}

#[tokio::test]
async fn api_key_client_does_not_retry_on_403() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/x"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(serde_json::json!({"description":"denied"})),
        )
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "key").unwrap();
    let err = client
        .get::<serde_json::Value>("/nests/x", &[])
        .await
        .unwrap_err();
    assert!(matches!(err, NestrError::Permission(_)));
}

const READ_ONLY_DENIAL: &str =
    "This API key is read-only. Use a key without the read-only setting to make changes.";

#[tokio::test]
async fn read_only_key_denial_gets_a_clear_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(serde_json::json!({"status":"error","message": READ_ONLY_DENIAL})),
        )
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "key").unwrap();
    let err = client
        .post::<serde_json::Value>("/nests", &serde_json::json!({"title":"T"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("this API key is read-only"), "{err}");
    assert!(err.contains("--read-only"), "{err}");
    assert!(!err.contains("auth status"), "{err}");
}

#[tokio::test]
async fn read_only_denial_does_not_trigger_a_token_refresh() {
    let server = MockServer::start().await;
    mount_token_endpoint(&server, 0).await;
    Mock::given(method("POST"))
        .and(path("/nests"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(serde_json::json!({"status":"error","message": READ_ONLY_DENIAL})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = NestrClient::with_refresh(
        server.uri(),
        "tok",
        Some(refresh_for(&server, "read-only-test-profile")),
    )
    .unwrap();
    let err = client
        .post::<serde_json::Value>("/nests", &serde_json::json!({"title":"T"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("this API key is read-only"), "{err}");
}

/// Write a minimal file-storage OAuth profile holding `access` + "old-refresh".
fn save_oauth_profile(name: &str, host: &str, access: &str, expiry: Option<u64>) {
    use nestr_cli::config::{save_profile, AuthKind, Profile};
    let profile = Profile {
        auth: AuthKind::OAuth,
        credential_storage: CredentialStorage::File,
        host: host.to_string(),
        workspace_id: "ws".into(),
        api_key: None,
        oauth_client_id: Some("client-1".into()),
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: Some(StoredOAuthTokens {
            access_token: access.into(),
            refresh_token: Some("old-refresh".into()),
            id_token: None,
            expiry,
        }),
        client_instance_id: None,
        default_output_format: None,
    };
    save_profile(name, &profile).unwrap();
}
