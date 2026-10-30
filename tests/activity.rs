use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::activity;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn mine_unwraps_the_activity_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/activity"))
        .and(query_param("limit", "20"))
        .and(query_param("withUser", "u7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"activity": [
                {"date": "2026-10-14T09:30:00.000Z", "workspaceId": "ws1", "nestId": "n1",
                 "nestTitle": "Website", "type": "comment", "description": "commented"}
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, None, Some(20), Some("u7"))
        .await
        .unwrap();
    assert_eq!(data.as_array().unwrap().len(), 1);
    assert_eq!(data[0]["nestTitle"], "Website");
}

#[tokio::test]
async fn another_users_activity_uses_their_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/u9/activity"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "data": {"activity": [
                {"date": "2026-10-14T09:30:00.000Z", "nestId": "n2", "nestTitle": "Ops",
                 "type": "consideration", "summary": "Triaged the inbox", "tools": ["search"],
                 "outcome": "done"}
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, Some("u9"), None, None)
        .await
        .unwrap();
    assert_eq!(data[0]["type"], "consideration");
}

#[tokio::test]
async fn a_missing_activity_key_is_an_empty_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/me/activity"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status": "success", "data": {}})),
        )
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let data = activity::fetch(&client, None, None, None).await.unwrap();
    assert_eq!(data, json!([]));
}

#[test]
fn a_row_says_what_happened() {
    let consideration = json!({
        "type": "consideration", "summary": "Triaged the inbox",
        "tools": ["search", "update"], "outcome": "done"
    });
    assert_eq!(
        activity::what(&consideration),
        "Triaged the inbox (tools: search, update; outcome: done)"
    );
    let plain = json!({"type": "comment", "description": "<p>commented on it</p>"});
    assert_eq!(activity::what(&plain), "commented on it");
}

// ---- nest activity: GET /nests/:id/activity --------------------------------

use activity::{Follower, NestActivityQuery};
use wiremock::matchers::query_param_is_missing;

fn row(id: &str) -> serde_json::Value {
    json!({"id": id, "date": "2026-10-29T09:00:00.000Z", "type": "change", "userId": "u1",
           "nestId": "n1", "nestTitle": "Review", "description": "changed it", "changes": []})
}

/// One page of the list envelope; `next` is the cursor carried on `links.next`.
fn page(ids: &[&str], next: Option<&str>) -> ResponseTemplate {
    let mut body = json!({
        "status": "success",
        "data": ids.iter().map(|i| row(i)).collect::<Vec<_>>(),
        "meta": {"has_more": next.is_some()},
        "links": {}
    });
    if let Some(c) = next {
        body["links"]["next"] = json!(format!(
            "https://app.nestr.io/api/nests/n1/activity?limit=2&before={c}"
        ));
    }
    ResponseTemplate::new(200).set_body_json(body)
}

fn ids(entries: &[serde_json::Value]) -> Vec<String> {
    entries
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

fn q(limit: Option<u32>, depth: Option<&str>) -> NestActivityQuery {
    NestActivityQuery {
        limit,
        depth: depth.map(str::to_string),
    }
}

#[tokio::test]
async fn nest_activity_passes_limit_and_depth_and_reads_the_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("limit", "5"))
        .and(query_param("depth", "all"))
        .and(query_param_is_missing("before"))
        .respond_with(page(&["e2", "e1"], Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let p = activity::fetch_nest_page(&client, "n1", &q(Some(5), Some("all")), None)
        .await
        .unwrap();
    assert_eq!(ids(&p.entries), ["e2", "e1"]);
    assert_eq!(p.entries[0]["nestTitle"], "Review");
    assert_eq!(p.next.as_deref(), Some("c1"));
}

#[tokio::test]
async fn nest_activity_without_more_pages_has_no_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("before", "c9"))
        .and(query_param("depth", "2"))
        .respond_with(page(&["e1"], None))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let p = activity::fetch_nest_page(&client, "n1", &q(None, Some("2")), Some("c9"))
        .await
        .unwrap();
    assert_eq!(ids(&p.entries), ["e1"]);
    assert_eq!(p.next, None);
}

#[tokio::test]
async fn since_follows_next_pages_until_it_reaches_the_entry() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param_is_missing("before"))
        .respond_with(page(&["e6", "e5"], Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("before", "c1"))
        .respond_with(page(&["e4", "e3"], Some("c2")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("before", "c2"))
        .respond_with(page(&["e2", "e1"], Some("c3")))
        .expect(1)
        .mount(&server)
        .await;
    // The entry is on page three, so page four is never asked for.
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("before", "c3"))
        .respond_with(page(&["e0"], None))
        .expect(0)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let s = activity::fetch_nest_since(&client, "n1", &q(Some(2), None), "e2", 20)
        .await
        .unwrap();
    assert!(s.reached);
    assert_eq!(ids(&s.entries), ["e6", "e5", "e4", "e3"]);
}

#[tokio::test]
async fn since_the_newest_entry_is_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .respond_with(page(&["e2", "e1"], Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let s = activity::fetch_nest_since(&client, "n1", &q(None, None), "e2", 20)
        .await
        .unwrap();
    assert!(s.reached);
    assert!(s.entries.is_empty());
}

#[tokio::test]
async fn since_stops_at_the_page_cap_when_the_entry_never_shows() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param_is_missing("before"))
        .respond_with(page(&["x1"], Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    for (before, id, next) in [("c1", "x2", "c2"), ("c2", "x3", "c3"), ("c3", "x4", "c4")] {
        Mock::given(method("GET"))
            .and(path("/nests/n1/activity"))
            .and(query_param("before", before))
            .respond_with(page(&[id], Some(next)))
            // Three pages are the cap: the fourth is never asked for.
            .expect(u64::from(before != "c3"))
            .mount(&server)
            .await;
    }
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let s = activity::fetch_nest_since(&client, "n1", &q(None, None), "gone", 3)
        .await
        .unwrap();
    assert!(!s.reached);
    assert!(s.capped);
    assert_eq!(s.entries.len(), 3);
}

#[tokio::test]
async fn since_stops_when_the_server_repeats_a_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .respond_with(page(&["x"], Some("again")))
        .expect(2)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let s = activity::fetch_nest_since(&client, "n1", &q(None, None), "gone", 20)
        .await
        .unwrap();
    assert!(!s.reached);
}

#[tokio::test]
async fn since_stops_when_the_list_ends() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .respond_with(page(&["e2", "e1"], None))
        .expect(1)
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let s = activity::fetch_nest_since(&client, "n1", &q(None, None), "gone", 20)
        .await
        .unwrap();
    assert!(!s.reached);
    assert!(!s.capped);
    assert_eq!(ids(&s.entries), ["e2", "e1"]);
}

/// Mount a one-shot response; wiremock picks the first mounted mock that still
/// matches, so a stack of these plays back in order.
async fn once(server: &MockServer, resp: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .respond_with(resp)
        .up_to_n_times(1)
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn follow_starts_after_the_newest_entry_and_never_repeats_one() {
    let server = MockServer::start().await;
    // Baseline at start: the newest entry, which is not printed.
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .and(query_param("limit", "1"))
        .respond_with(page(&["e1"], Some("b")))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    once(&server, page(&["e3", "e2", "e1"], None)).await;
    once(&server, page(&["e3", "e2", "e1"], None)).await;
    // e3 vanished (deleted), so the anchor is not found: e2 and e1 come back
    // again and must not be printed a second time.
    once(&server, page(&["e4", "e2", "e1"], None)).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), None)
        .await
        .unwrap();
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e2", "e3"]);
    assert!(f.poll(&client).await.unwrap().is_empty());
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e4"]);
}

#[tokio::test]
async fn follow_from_since_pages_back_and_prints_oldest_first() {
    let server = MockServer::start().await;
    once(&server, page(&["e5", "e4"], Some("c1"))).await;
    once(&server, page(&["e3", "e2"], Some("c2"))).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(Some(2), None), Some("e2".into()))
        .await
        .unwrap();
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e3", "e4", "e5"]);
}

#[tokio::test]
async fn follow_retries_a_failed_poll_without_losing_entries() {
    let server = MockServer::start().await;
    once(&server, page(&["e1"], None)).await;
    once(&server, ResponseTemplate::new(500).set_body_string("boom")).await;
    once(&server, page(&["e3", "e2", "e1"], None)).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), None)
        .await
        .unwrap();
    assert!(f.poll(&client).await.is_err());
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e2", "e3"]);
}

#[test]
fn depth_is_a_number_or_all() {
    assert_eq!(activity::parse_depth("all").unwrap(), "all");
    assert_eq!(activity::parse_depth("0").unwrap(), "0");
    assert_eq!(activity::parse_depth("3").unwrap(), "3");
    assert!(activity::parse_depth("-1").is_err());
    assert!(activity::parse_depth("deep").is_err());
}

#[test]
fn the_cursor_is_read_from_the_next_link() {
    assert_eq!(
        activity::cursor_from_next("https://h/api/nests/n1/activity?limit=2&before=abc%3D%3D")
            .as_deref(),
        Some("abc==")
    );
    assert_eq!(
        activity::cursor_from_next("https://h/api/nests/n1/activity"),
        None
    );
    assert_eq!(activity::cursor_from_next("not a url"), None);
}

#[test]
fn follow_retries_only_what_a_retry_can_fix() {
    use nestr_cli::error::NestrError;
    let api = |status| NestrError::Api {
        status,
        message: "x".into(),
    };
    assert!(activity::is_transient(&api(500)));
    assert!(activity::is_transient(&api(503)));
    assert!(activity::is_transient(&api(429)));
    assert!(!activity::is_transient(&NestrError::Auth("revoked".into())));
    assert!(!activity::is_transient(&NestrError::Permission(
        "no".into()
    )));
    assert!(!activity::is_transient(&NestrError::NotFound(
        "gone".into()
    )));
    assert!(!activity::is_transient(&NestrError::Validation(
        "bad".into()
    )));
}

/// The binary, `--follow`, against a server that rejects the key: it must exit
/// with the error instead of retrying forever.
#[tokio::test]
async fn follow_exits_when_the_credential_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": "revoked"})))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let mut child = nestr(&home, &["--follow", "--since", "e1", "--interval", "1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(20), child.wait())
        .await
        .expect("--follow kept retrying a rejected credential")
        .unwrap();
    assert!(!status.success());
}

/// A page whose rows carry their own minute, so entries have distinct dates.
fn dated_page(rows: &[(&str, u32)]) -> ResponseTemplate {
    let data: Vec<serde_json::Value> = rows
        .iter()
        .map(|(id, min)| {
            let mut r = row(id);
            r["date"] = json!(format!("2026-10-29T09:{min:02}:00.000Z"));
            r
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(json!({
        "status": "success", "data": data, "meta": {"has_more": false}, "links": {}
    }))
}

#[tokio::test]
async fn follow_never_prints_history_from_before_it_started() {
    let server = MockServer::start().await;
    once(&server, dated_page(&[("e3", 3)])).await;
    // The baseline e3 was deleted before anything new arrived: the poll falls
    // back to everything it read, but e2 and e1 are older than the start.
    once(
        &server,
        dated_page(&[("e5", 5), ("e4", 4), ("e2", 2), ("e1", 1)]),
    )
    .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), None)
        .await
        .unwrap();
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e4", "e5"]);
    assert_eq!(f.missed_anchor.as_deref(), Some("e3"));
}

#[tokio::test]
async fn follow_from_since_learns_the_anchor_date_once_it_sees_it() {
    let server = MockServer::start().await;
    // --since e3: the first poll finds it with nothing newer.
    once(&server, dated_page(&[("e3", 3), ("e2", 2)])).await;
    // Then e3 is deleted; e2 is older than it and must stay unprinted.
    once(&server, dated_page(&[("e4", 4), ("e2", 2)])).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), Some("e3".into()))
        .await
        .unwrap();
    assert!(f.poll(&client).await.unwrap().is_empty());
    assert_eq!(ids(&f.poll(&client).await.unwrap()), ["e4"]);
}

#[tokio::test]
async fn follow_moves_past_a_deleted_anchor_even_when_nothing_is_new() {
    let server = MockServer::start().await;
    once(&server, dated_page(&[("e3", 3)])).await;
    once(&server, dated_page(&[("e2", 2), ("e1", 1)])).await;
    // The next poll must look for e2 now, and find it, instead of missing e3 again.
    Mock::given(method("GET"))
        .and(path("/nests/n1/activity"))
        .respond_with(dated_page(&[("e2", 2), ("e1", 1)]))
        .mount(&server)
        .await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), None)
        .await
        .unwrap();
    assert!(f.poll(&client).await.unwrap().is_empty());
    assert_eq!(f.missed_anchor.take().as_deref(), Some("e3"));
    assert!(f.poll(&client).await.unwrap().is_empty());
    assert_eq!(f.missed_anchor, None);
}

// ---- the binary: exit codes, the first read, a closed pipe -----------------

/// A NESTR_HOME holding a file-stored API-key profile pointed at `server`.
fn profile_home(server: &MockServer) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    let profiles = home.path().join(".nestr").join("profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    std::fs::write(
        profiles.join("default.toml"),
        format!(
            "auth = \"api_key\"\ncredential_storage = \"file\"\nhost = \"{}\"\n\
             workspace_id = \"ws1\"\napi_key = \"test-key\"\n",
            server.uri()
        ),
    )
    .unwrap();
    home
}

/// `nestr nests activity n1 <args>` with that profile; stderr piped.
fn nestr(home: &tempfile::TempDir, args: &[&str]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_nestr"));
    cmd.args(["nests", "activity", "n1"])
        .args(args)
        .env("NESTR_HOME", home.path())
        .env_remove("NESTR_API_KEY")
        .env_remove("NESTR_HOST")
        .env_remove("NESTR_PROFILE")
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    cmd
}

/// Every call answers `[e{n+1}, e{n}]` for the n-th call, so each poll finds its
/// anchor and has exactly one new entry. Paths are under `/api`.
struct OneNewPerCall(std::sync::atomic::AtomicUsize);

impl wiremock::Respond for OneNewPerCall {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
        dated_page(&[(&format!("e{}", n + 1), n + 1), (&format!("e{n}"), n)])
    }
}

/// Pages that never end: the n-th call answers `[x{n}]` with a fresh cursor.
struct Endless(std::sync::atomic::AtomicUsize);

impl wiremock::Respond for Endless {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        page(&[&format!("x{n}")], Some(&format!("c{n}")))
    }
}

async fn output_of(mut cmd: tokio::process::Command) -> std::process::Output {
    cmd.stdout(std::process::Stdio::piped());
    tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output())
        .await
        .expect("nestr did not finish")
        .unwrap()
}

#[tokio::test]
async fn since_a_missing_entry_prints_nothing_and_exits_4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(page(&["e2", "e1"], None))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let out = output_of(nestr(&home, &["--since", "gone", "-o", "json"])).await;
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(activity::EXIT_ANCHOR_NOT_FOUND));
    assert_eq!(activity::EXIT_ANCHOR_NOT_FOUND, 4);
    assert!(out.stdout.is_empty(), "printed: {:?}", out.stdout);
    assert!(
        stderr.contains("'gone' is not in this nest's activity"),
        "{stderr}"
    );
    assert!(!stderr.contains("pages"), "{stderr}");
}

#[tokio::test]
async fn since_beyond_the_page_cap_says_so_and_exits_4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(Endless(Default::default()))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let out = output_of(nestr(
        &home,
        &["--since", "gone", "--limit", "1", "-o", "json"],
    ))
    .await;
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(4));
    assert!(out.stdout.is_empty());
    assert!(
        stderr.contains("not found in the newest 20 pages"),
        "{stderr}"
    );
}

#[tokio::test]
async fn follow_from_a_since_entry_that_is_gone_exits_4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(page(&["e2", "e1"], None))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let out = output_of(nestr(
        &home,
        &["--follow", "--since", "gone", "--interval", "1"],
    ))
    .await;
    assert_eq!(out.status.code(), Some(4));
    assert!(out.stdout.is_empty());
}

#[tokio::test]
async fn a_since_miss_is_not_retried() {
    let server = MockServer::start().await;
    once(&server, page(&["e2", "e1"], None)).await;
    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let mut f = Follower::start(&client, "n1", q(None, None), Some("gone".into()))
        .await
        .unwrap();
    let err = f.poll(&client).await.unwrap_err();
    assert!(
        matches!(err, nestr_cli::error::NestrError::AnchorNotFound(_)),
        "{err:?}"
    );
    assert!(!activity::is_transient(&err));
}

#[tokio::test]
async fn follow_retries_a_failed_first_read() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({"message": "deploying"})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(OneNewPerCall(Default::default()))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let mut child = nestr(&home, &["--follow", "--interval", "1"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdout));
    let first = tokio::time::timeout(std::time::Duration::from_secs(20), lines.next_line())
        .await
        .expect("--follow gave up after a failed first read")
        .unwrap()
        .expect("--follow exited after a failed first read");
    assert!(first.contains("\"id\":\"e2\""), "{first}");
}

#[tokio::test]
async fn follow_exits_quietly_when_stdout_closes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/nests/n1/activity"))
        .respond_with(OneNewPerCall(Default::default()))
        .mount(&server)
        .await;
    let home = profile_home(&server);
    let mut child = nestr(&home, &["--follow", "--interval", "1"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdout));
    tokio::time::timeout(std::time::Duration::from_secs(20), lines.next_line())
        .await
        .expect("no first line")
        .unwrap();
    // The consumer goes away; the next entry hits a closed pipe.
    drop(lines);
    let out = tokio::time::timeout(std::time::Duration::from_secs(20), child.wait_with_output())
        .await
        .expect("--follow kept running after stdout closed")
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("Broken pipe"), "{stderr}");
}

#[test]
fn backoff_doubles_after_each_failure_up_to_a_cap() {
    use std::time::Duration;
    let s = Duration::from_secs;
    assert_eq!(activity::backoff(s(120), 0), s(120));
    assert_eq!(activity::backoff(s(120), 1), s(240));
    assert_eq!(activity::backoff(s(120), 2), s(480));
    assert_eq!(activity::backoff(s(120), 3), s(600));
    assert_eq!(activity::backoff(s(120), 40), s(600));
    // An interval above the cap is never shortened.
    assert_eq!(activity::backoff(s(900), 3), s(900));
}
