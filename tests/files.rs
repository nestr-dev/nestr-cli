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

#[tokio::test]
async fn upload_posts_name_type_and_base64_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/nests/n1/files"))
        .and(body_json(json!({
            "name": "data.csv",
            "contentType": "text/csv",
            "dataBase64": b64(b"a,b\n1,2\n"),
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"id": "f9", "name": "data.csv", "contentType": "text/csv", "size": 8}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = files::upload_file(&client, "n1", "data.csv", "text/csv", b"a,b\n1,2\n")
        .await
        .unwrap();
    assert_eq!(data["id"], "f9");
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
    Mock::given(method("POST"))
        .and(path("/api/nests/n1/files"))
        .and(body_json(json!({
            "name": "plan.md",
            "contentType": "text/markdown",
            "dataBase64": b64(b"# Plan\n"),
        })))
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
    assert!(String::from_utf8_lossy(&out.stdout).contains("f9"));
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
    assert!(server.received_requests().await.unwrap().is_empty());
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
    assert!(server.received_requests().await.unwrap().is_empty());
}
