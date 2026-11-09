use nestr_cli::api_client::NestrClient;
use nestr_cli::commands::search;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn workspace_search_hits_ws_path_and_unwraps() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/search"))
        .and(query_param("search", "hello"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "data": [{"_id":"n1","title":"Hello world"}],
            "meta": {"page":1,"total_pages":1,"total":1}
        })))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _meta, _warnings) = search::run_search(&client, "ws1", "hello", None, &[])
        .await
        .unwrap();
    assert_eq!(
        data,
        serde_json::json!([{"_id":"n1","title":"Hello world"}])
    );
}

#[tokio::test]
async fn nest_scoped_search_hits_nest_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/nests/n9/search"))
        .and(query_param("search", "todo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status":"success","data":[]
        })))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (data, _meta, _warnings) = search::run_search(&client, "ws1", "todo", Some("n9"), &[])
        .await
        .unwrap();
    assert_eq!(data, serde_json::json!([]));
}

#[tokio::test]
async fn search_surfaces_the_warnings_the_server_attached() {
    // The API reports a filter it could not apply instead of silently widening or
    // emptying the result. unwrap_data keeps only data/meta/links, so these used
    // to be dropped before any caller could see them.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "meta": {"page":1,"total_pages":1,"total":0},
            "searchWarnings": [
                {"code":"malformed_in_value","message":"'not-an-id' is not a nest id in 'in:not-an-id'."}
            ],
            "data": []
        })))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (_data, _meta, warnings) = search::run_search(&client, "ws1", "in:not-an-id", None, &[])
        .await
        .unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("not-an-id"));
}

#[tokio::test]
async fn a_clean_search_surfaces_no_warnings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspaces/ws1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "meta": {"page":1,"total_pages":1,"total":1},
            "data": [{"_id":"n1","title":"Hello"}]
        })))
        .mount(&server)
        .await;

    let client = NestrClient::new(server.uri(), "tok").unwrap();
    let (_data, _meta, warnings) = search::run_search(&client, "ws1", "hello", None, &[])
        .await
        .unwrap();
    assert!(warnings.is_empty());
}
