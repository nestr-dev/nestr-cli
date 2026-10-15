//! Response notices (`warnings`, `searchWarnings`, `appliedDefaults`, top-level
//! `hints`) are pulled out of every response by the client and printed to stderr,
//! while stdout keeps only the unwrapped data.
use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::{nests, search};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const IGNORED: &str = "Ignored unsupported parameter: foo. It was not saved.";

#[tokio::test]
async fn write_warnings_are_collected_and_data_still_unwraps() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"n1","title":"T"},"warnings":[IGNORED]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = nests::create_nest(&client, &json!({"title":"T"}))
        .await
        .unwrap();
    assert_eq!(data, json!({"_id":"n1","title":"T"}));
    let notices = client.notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0]["warnings"], json!([IGNORED]));
}

#[tokio::test]
async fn search_warnings_are_collected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta":{"page":1,"total_pages":1,"total":0},
            "searchWarnings":[{"type":"unknown_label","message":"Unknown label 'x' in 'label:x'. This filter was ignored."}],
            "data":[]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _) = search::run_search(&client, "ws1", "label:x", None, &[])
        .await
        .unwrap();
    assert_eq!(data, json!([]));
    assert!(client.notices()[0].contains_key("searchWarnings"));
}

#[tokio::test]
async fn children_applied_defaults_are_collected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/c1/children"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","meta":{"page":1,"total_pages":1,"total":0},
            "appliedDefaults":[{"operator":"depth:1","reason":"r","override":"o"}],
            "data":[]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    nests::fetch_children(&client, "c1", &[("search", "label:role")])
        .await
        .unwrap();
    assert_eq!(
        client.notices()[0]["appliedDefaults"][0]["operator"],
        "depth:1"
    );
}

#[tokio::test]
async fn a_response_without_notices_records_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"n1","hints":[{"label":"per-nest hint"}]}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    nests::fetch_get(&client, "n1", &[]).await.unwrap();
    assert!(client.notices().is_empty());
}

/// Run the real binary against the mock server with an api-key profile.
fn run_cli(host: &str, args: &[&str]) -> std::process::Output {
    use nestr_cli::config::{save_profile, AuthKind, CredentialStorage, Profile};
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("NESTR_HOME", tmp.path());
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
    save_profile("t", &profile).unwrap();
    std::env::remove_var("NESTR_HOME");
    std::process::Command::new(env!("CARGO_BIN_EXE_nestr"))
        .args(["--profile", "t", "--yes"])
        .args(args)
        .env("NESTR_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .env_remove("NESTR_API_KEY")
        .env_remove("NESTR_PROFILE")
        .output()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_prints_warnings_to_stderr_and_keeps_stdout_clean() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/nests/n1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"success","data":{"_id":"n1","title":"T"},"warnings":[IGNORED]
        })))
        .mount(&server)
        .await;

    let out = run_cli(
        &server.uri(),
        &["-o", "text", "nests", "update", "n1", "--title", "T"],
    );
    let (stdout, stderr) = (
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(out.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains(&format!("warning: {IGNORED}")),
        "stderr: {stderr}"
    );
    assert!(!stdout.contains("Ignored"), "stdout: {stdout}");

    let out = run_cli(
        &server.uri(),
        &["-o", "json", "nests", "update", "n1", "--title", "T"],
    );
    let stdout: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stdout, json!({"_id":"n1","title":"T"}));
    let stderr = String::from_utf8_lossy(&out.stderr);
    let line = stderr
        .lines()
        .find(|l| l.starts_with('{'))
        .expect("a JSON notice line on stderr");
    let notice: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(notice, json!({"warnings":[IGNORED]}));
}
