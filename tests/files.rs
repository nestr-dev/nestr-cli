use base64::Engine as _;
use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::files;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn list_unwraps_the_descriptors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": [{"id": "f1", "name": "notes.md", "contentType": "text/markdown", "size": 12}]
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::list_files(&client, "n1", None, None).await.unwrap();
    assert_eq!(data[0]["id"], "f1");
    assert_eq!(data[0]["name"], "notes.md");
}

#[tokio::test]
async fn list_passes_the_group_and_search() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files"))
        .and(query_param("context", "nestradamus_files"))
        .and(query_param("search", "leave policy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": [{"id": "f2", "name": "handbook.pdf", "matches": [{"excerpt": "Leave is…", "offset": 4100}]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::list_files(
        &client,
        "ws1",
        Some("nestradamus_files"),
        Some("leave policy"),
    )
    .await
    .unwrap();
    assert_eq!(data[0]["matches"][0]["offset"], 4100);
}

#[tokio::test]
async fn read_returns_the_file_with_its_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f1", "name": "a.txt", "contentType": "text/plain", "size": 5, "dataBase64": b64(b"hello")}
        })))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::read_file(&client, "n1", "f1").await.unwrap();
    assert_eq!(files::decode_bytes(&data).unwrap(), b"hello");
}

#[tokio::test]
async fn read_text_asks_for_extracted_text_from_an_offset() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/ws1/files/f2"))
        .and(query_param("as", "text"))
        .and(query_param("offset", "4100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f2", "name": "handbook.pdf", "contentType": "text/plain",
                     "text": "Leave is…", "offset": 4100, "total": 9000, "nextOffset": 8100}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::read_file_text(&client, "ws1", "f2", Some(4100))
        .await
        .unwrap();
    assert_eq!(data["nextOffset"], 8100);
}

/// A temp dir holding `name` with `bytes`; keep the dir alive while the path is used.
fn local_file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join(name);
    std::fs::write(&p, bytes).unwrap();
    (dir, p)
}

/// Serve `POST {api}/nests/n1/files/upload-url` handing out a URL on the same mock.
async fn mount_upload_url(server: &MockServer, api: &str, request: serde_json::Value, max: u64) {
    Mock::given(method("POST"))
        .and(path(format!("{api}/nests/n1/files/upload-url")))
        .and(body_json(request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {
                "url": format!("{}/api/file-uploads/n1/tok123", server.uri()),
                "method": "POST",
                "expiresAt": "2026-11-10T14:00:00.000Z",
                "maxBytes": max,
                "curl": "curl --data-binary @<path-to-file> '...'"
            }
        })))
        .expect(1)
        .mount(server)
        .await;
}

/// What the CLI sent, minus the bodiless `HEAD /` probes a local port scanner can
/// land on a listening mock; the CLI never sends HEAD.
async fn sent(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method != wiremock::http::Method::HEAD)
        .collect()
}

fn uploads(reqs: &[wiremock::Request]) -> Vec<&wiremock::Request> {
    reqs.iter()
        .filter(|r| r.url.path().starts_with("/api/file-uploads/"))
        .collect()
}

#[tokio::test]
async fn upload_sends_the_raw_bytes_to_a_single_use_url_without_the_key() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "data.csv", "contentType": "text/csv"}),
        10 * 1024 * 1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f9", "name": "data.csv", "contentType": "text/csv", "size": 8}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (_dir, src) = local_file("data.csv", b"a,b\n1,2\n");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::upload_file(&client, "n1", "data.csv", "text/csv", &src, false)
        .await
        .unwrap();
    assert_eq!(data["id"], "f9");

    let reqs = server.received_requests().await.unwrap();
    let up = uploads(&reqs);
    assert_eq!(up.len(), 1);
    let up = up[0];
    assert_eq!(
        up.body, b"a,b\n1,2\n",
        "the body is the file's bytes, not base64 or JSON"
    );
    assert!(
        up.headers.get("authorization").is_none(),
        "the upload URL is the credential; the API key must not go along"
    );
    assert_eq!(up.headers.get("content-type").unwrap(), "text/csv");
    assert_eq!(up.headers.get("content-length").unwrap(), "8");
    assert!(
        up.headers.get("transfer-encoding").is_none(),
        "sent with a length, not chunked"
    );
}

#[tokio::test]
async fn upload_inline_asks_for_an_inline_image_and_returns_its_reference() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "shot.png", "contentType": "image/png", "inline": true}),
        10 * 1024 * 1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {
                "id": "f3", "name": "shot.png", "contentType": "image/png", "size": 3,
                "context": "inline_image",
                "markdown": "![shot.png](/file/download?id=f3&name=shot.png)"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (_dir, src) = local_file("shot.png", b"png");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::upload_file(&client, "n1", "shot.png", "image/png", &src, true)
        .await
        .unwrap();
    assert_eq!(
        data["markdown"],
        "![shot.png](/file/download?id=f3&name=shot.png)"
    );
}

#[tokio::test]
async fn upload_inline_refusal_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/files/upload-url"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "status": "error",
            "message": "An inline upload must be a PNG, JPEG, GIF or WebP image, not \"text/plain\""
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"a");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, true)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("PNG, JPEG, GIF or WebP"), "{err}");
}

#[tokio::test]
async fn upload_over_the_servers_limit_is_refused_before_sending() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "big.bin", "contentType": "application/octet-stream"}),
        4,
    )
    .await;
    let (_dir, src) = local_file("big.bin", b"12345");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(
        &client,
        "n1",
        "big.bin",
        "application/octet-stream",
        &src,
        false,
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too large"), "{msg}");
    assert!(msg.contains("4 B"), "names the limit: {msg}");
    let reqs = server.received_requests().await.unwrap();
    assert!(uploads(&reqs).is_empty(), "no bytes go out");
}

#[tokio::test]
async fn upload_of_an_empty_file_is_refused_before_asking_for_a_url() {
    let server = MockServer::start().await;
    let (_dir, src) = local_file("empty.txt", b"");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "empty.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("empty"), "{err}");
    assert!(sent(&server).await.is_empty());
}

#[tokio::test]
async fn upload_413_says_the_limit_the_server_names() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.bin", "contentType": "application/octet-stream"}),
        10 * 1024 * 1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(413).set_body_json(json!({
            "status": "error",
            "message": "File \"a.bin\" exceeds the maximum upload size of 2048 bytes"
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.bin", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(
        &client,
        "n1",
        "a.bin",
        "application/octet-stream",
        &src,
        false,
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too large"), "{msg}");
    assert!(
        msg.contains("2.0 KB"),
        "the limit from the server's message: {msg}"
    );
    assert!(
        !msg.contains("tok123"),
        "the upload URL stays out of errors: {msg}"
    );
}

#[tokio::test]
async fn upload_413_from_a_proxy_falls_back_to_the_issued_limit() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.bin", "contentType": "application/octet-stream"}),
        3 * 1024 * 1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(
            ResponseTemplate::new(413)
                .set_body_string("<html><body>413 Request Entity Too Large</body></html>"),
        )
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.bin", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(
        &client,
        "n1",
        "a.bin",
        "application/octet-stream",
        &src,
        false,
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too large"), "{msg}");
    assert!(msg.contains("3.0 MB"), "{msg}");
    assert!(!msg.contains("<html>"), "no raw proxy page: {msg}");
}

#[tokio::test]
async fn upload_to_a_used_or_expired_url_says_to_run_it_again() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(410).set_body_json(json!({
            "status": "error",
            "message": "This upload URL has expired. Request a new one."
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("has expired"),
        "keeps the server's reason: {msg}"
    );
    assert!(msg.contains("again"), "says what to do: {msg}");
    assert!(!msg.contains("tok123"), "{msg}");
}

#[tokio::test]
async fn upload_404_keeps_the_reason_and_only_suggests_a_retry_if_the_nest_exists() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "status": "error",
            "message": "Nest not found"
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Nest not found"), "{msg}");
    assert!(msg.contains("If the nest still exists"), "{msg}");
}

#[tokio::test]
async fn upload_refusal_without_a_json_reason_shows_no_proxy_page() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(410).set_body_string("<html>Gone</html>"))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("<html>"), "{msg}");
    assert!(msg.contains("HTTP 410"), "{msg}");
}

#[tokio::test]
async fn upload_bare_403_reads_as_a_refusal_not_a_fault() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Server error."))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("refused this request without saying why"),
        "{msg}"
    );
}

#[tokio::test]
async fn upload_401_is_not_blamed_on_the_login() {
    // No API key goes to the upload URL, so a 401 there says nothing about the login.
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("upload URL was not accepted"), "{msg}");
    assert!(
        !msg.contains("auth login") && !msg.contains("API key"),
        "{msg}"
    );
}

#[tokio::test]
async fn upload_refused_at_upload_time_keeps_the_servers_reason() {
    // The upload re-checks rights and the nest, so a 403 or 404 there is not
    // necessarily about the URL.
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "",
        json!({"name": "a.txt", "contentType": "text/plain"}),
        1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "status": "error",
            "message": "Not allowed to change files on this nest"
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("Not allowed to change files"),
        "{err}"
    );
}

#[tokio::test]
async fn upload_refuses_a_cleartext_upload_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/files/upload-url"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"url": "http://uploads.example.com/api/file-uploads/n1/tok123",
                     "method": "POST", "maxBytes": 1024}
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unencrypted") || msg.contains("https"),
        "{msg}"
    );
    assert!(!msg.contains("tok123"), "{msg}");
}

#[tokio::test]
async fn upload_on_a_server_without_upload_urls_says_so() {
    // An older server routes the path to the file route, which has no POST.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/files/upload-url"))
        .respond_with(ResponseTemplate::new(405).set_body_json(json!({
            "status": "error",
            "message": "API endpoint does not exist"
        })))
        .mount(&server)
        .await;
    let (_dir, src) = local_file("a.txt", b"abc");
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let err = files::upload_file(&client, "n1", "a.txt", "text/plain", &src, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("upload URL"), "{err}");
}

#[tokio::test]
async fn delete_removes_the_file() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/nests/n1/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "success"})))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    files::delete_file(&client, "n1", "f1").await.unwrap();
}

#[test]
fn content_type_is_guessed_from_the_extension() {
    assert_eq!(files::guess_content_type("notes.md"), "text/markdown");
    assert_eq!(files::guess_content_type("Photo.JPG"), "image/jpeg");
    assert_eq!(files::guess_content_type("report.pdf"), "application/pdf");
    assert_eq!(
        files::guess_content_type("blob.weird"),
        "application/octet-stream"
    );
    assert_eq!(
        files::guess_content_type("Makefile"),
        "application/octet-stream"
    );
}

// ---- end to end through the binary -------------------------------------------------

/// Run the binary against `server` with a throwaway API-key profile.
fn nestr(server: &MockServer, home: &std::path::Path, args: &[&str]) -> std::process::Output {
    let profiles = home.join(".nestr").join("profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    std::fs::write(
        profiles.join("t.toml"),
        format!(
            "auth = \"api_key\"\ncredential_storage = \"file\"\nhost = \"{}\"\nworkspace_id = \"ws1\"\napi_key = \"tok\"\n",
            server.uri()
        ),
    )
    .unwrap();
    // The mock serves under /api, the same base a real host gets.
    std::process::Command::new(env!("CARGO_BIN_EXE_nestr"))
        .args(["-p", "t"])
        .args(args)
        .env("NESTR_HOME", home)
        .env_remove("NESTR_API_KEY")
        .env_remove("NESTR_PROFILE")
        .env_remove("NESTR_READ_ONLY")
        .output()
        .unwrap()
}

#[tokio::test]
async fn read_prints_a_text_file_and_strips_terminal_escapes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f1", "name": "a.md", "contentType": "text/markdown", "size": 14,
                     "dataBase64": b64(b"# Title\n\x1b[31mred\n")}
        })))
        .mount(&server)
        .await;
    let home = tempfile::tempdir().unwrap();
    let out = nestr(
        &server,
        home.path(),
        &["nests", "files", "read", "n1", "f1"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("# Title\n"), "{stdout}");
    assert!(stdout.contains("red"), "{stdout}");
    assert!(
        !stdout.contains('\u{1b}'),
        "ESC must not reach the terminal"
    );
}

#[tokio::test]
async fn read_does_not_dump_a_binary_file_to_the_terminal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f1", "name": "pic.png", "contentType": "image/png", "size": 4,
                     "dataBase64": b64(&[0x89, b'P', b'N', b'G'])}
        })))
        .mount(&server)
        .await;
    let home = tempfile::tempdir().unwrap();
    let out = nestr(
        &server,
        home.path(),
        &["nests", "files", "read", "n1", "f1"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("pic.png") && stdout.contains("--save"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("PNG"),
        "bytes must not be printed: {stdout}"
    );
}

#[tokio::test]
async fn read_save_writes_the_decoded_bytes_and_refuses_to_overwrite() {
    let server = MockServer::start().await;
    let bytes: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0, 255];
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f1", "name": "pic.png", "contentType": "image/png", "size": 6,
                     "dataBase64": b64(&bytes)}
        })))
        .mount(&server)
        .await;
    let home = tempfile::tempdir().unwrap();
    let dest = home.path().join("out.png");
    let dest_s = dest.to_str().unwrap();
    let out = nestr(
        &server,
        home.path(),
        &["nests", "files", "read", "n1", "f1", "--save", dest_s],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    let again = nestr(
        &server,
        home.path(),
        &["nests", "files", "read", "n1", "f1", "--save", dest_s],
    );
    assert!(
        !again.status.success(),
        "an existing file must not be overwritten"
    );
    assert!(String::from_utf8_lossy(&again.stderr).contains("--force"));
}

#[tokio::test]
async fn upload_reads_the_file_and_names_it_after_the_path() {
    let server = MockServer::start().await;
    mount_upload_url(
        &server,
        "/api",
        json!({"name": "plan.md", "contentType": "text/markdown"}),
        10 * 1024 * 1024,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/file-uploads/n1/tok123"))
        .and(wiremock::matchers::body_bytes(b"# Plan\n".to_vec()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f9", "name": "plan.md", "contentType": "text/markdown", "size": 7}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let home = tempfile::tempdir().unwrap();
    let src = home.path().join("plan.md");
    std::fs::write(&src, b"# Plan\n").unwrap();
    let out = nestr(
        &server,
        home.path(),
        &["nests", "files", "upload", "n1", src.to_str().unwrap()],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Uploaded plan.md (7 B) to n1  [f9]"),
        "{stdout}"
    );
}

#[tokio::test]
async fn upload_is_blocked_in_read_only_mode() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let src = home.path().join("plan.md");
    std::fs::write(&src, b"# Plan\n").unwrap();
    let out = nestr(
        &server,
        home.path(),
        &[
            "--read-only",
            "nests",
            "files",
            "upload",
            "n1",
            src.to_str().unwrap(),
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("read-only"));
    assert!(sent(&server).await.is_empty());
}

#[tokio::test]
async fn delete_needs_yes_outside_a_terminal() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let out = nestr(
        &server,
        home.path(),
        &["nests", "files", "delete", "n1", "f1"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--yes"));
    assert!(sent(&server).await.is_empty());
}

#[test]
fn inline_reference_hint_prints_the_reference_verbatim() {
    // An alt text with an entity-like `&` and angle brackets must come out exactly as
    // the API returned it, or the copied reference differs from the stored one.
    let markdown = "![R&amp;D <v2>.png](/file/download?id=f3&name=R%26D.png)";
    let hint = files::inline_reference_hint(&json!({"id": "f3", "markdown": markdown})).unwrap();
    assert!(hint.ends_with(markdown), "{hint}");
    // Terminal control bytes are still kept out.
    let hostile = files::inline_reference_hint(&json!({"markdown": "![a\u{1b}[31m](/x)"})).unwrap();
    assert!(!hostile.contains('\u{1b}'), "{hostile}");
}

#[test]
fn inline_reference_hint_is_absent_for_a_plain_attachment() {
    assert!(files::inline_reference_hint(&json!({"id": "f1", "name": "a.txt"})).is_none());
}
