use nestr_cli::api_client::NestrClient;
use nestr_cli::config::CredentialStorage;
use nestr_cli::oauth::ReactiveRefresh;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn refreshes_and_retries_once_on_403() {
    let server = MockServer::start().await;

    // The token endpoint hands back a new access token.
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "fresh-token",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    // First call with the stale token → 403; after refresh, the fresh token → 200.
    Mock::given(method("GET"))
        .and(path("/nests/x"))
        .and(wiremock::matchers::header("authorization", "Bearer stale"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(serde_json::json!({"description":"nope"})),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/nests/x"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer fresh-token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"_id":"x"})))
        .mount(&server)
        .await;

    let refresh = ReactiveRefresh::new(
        format!("{}/oauth/token", server.uri()),
        "client-1".into(),
        // No profile is persisted; OsStore keyring write may no-op/err in CI, so use
        // a temp NESTR_HOME + File storage via a saved profile instead:
        "refresh-test-profile".into(),
        CredentialStorage::File,
        "old-refresh".into(),
    );
    // Persist a profile so File-storage persistence in perform() succeeds.
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
    save_oauth_profile("refresh-test-profile", &server.uri());

    let client = NestrClient::with_refresh(server.uri(), "stale", Some(refresh)).unwrap();
    let v: serde_json::Value = client.get("/nests/x", &[]).await.unwrap();
    assert_eq!(v["_id"], "x");
    std::env::remove_var("NESTR_HOME");
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
    assert!(matches!(err, nestr_cli::error::NestrError::Permission(_)));
}

/// Helper: write a minimal OAuth profile TOML so update_profile_oauth_tokens has a file to patch.
fn save_oauth_profile(name: &str, host: &str) {
    use nestr_cli::config::{save_profile, AuthKind, Profile};
    let profile = Profile {
        auth: AuthKind::OAuth,
        credential_storage: CredentialStorage::File,
        host: host.to_string(),
        workspace_id: "ws".into(),
        api_key: None,
        label: None,
        oauth_client_id: Some("client-1".into()),
        oauth_token_url: None,
        oauth_authorize_url: None,
        oauth_tokens: Some(nestr_cli::oauth::StoredOAuthTokens {
            access_token: "stale".into(),
            refresh_token: Some("old-refresh".into()),
            id_token: None,
            expiry: Some(0),
        }),
        default_output_format: None,
    };
    save_profile(name, &profile).unwrap();
}
