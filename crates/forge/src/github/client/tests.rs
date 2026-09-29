use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;

use super::Client;
use crate::{
    ForgeError,
    github::{
        testing::Fixtures,
        transport::TransportError,
    },
};

const QUERY: &str = "query Q { viewer { login } }";
const MUTATION: &str = "mutation M($input: X!) { m(input: $input) { id } }";

/// A client on `fixtures` that records its waits instead of sleeping, at the epoch second 1000.
fn client(fixtures: &Fixtures) -> (Client, Arc<Mutex<Vec<Duration>>>) {
    let waits = Arc::new(Mutex::new(Vec::new()));
    let log = waits.clone();
    let client = Client::new(fixtures.clone()).with_clock(move |d| log.lock().unwrap().push(d), || 1000);
    (client, waits)
}

fn data(login: &str) -> String {
    json!({"data": {"viewer": {"login": login}}}).to_string()
}

#[test]
fn a_good_answer_gives_its_data() {
    let (client, waits) = client(&Fixtures::new().ok("Q", data("ada")));
    let graph = client.graphql(QUERY, json!({})).unwrap();
    assert_eq!(graph.data["viewer"]["login"], "ada");
    assert!(waits.lock().unwrap().is_empty());
}

#[test]
fn a_primary_rate_limit_waits_until_the_reset_and_tries_again() {
    let limited = json!({"errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]}).to_string();
    let fixtures = Fixtures::new()
        .status("Q", 200, &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1010")], limited)
        .ok("Q", data("ada"));
    let (client, waits) = client(&fixtures);
    assert_eq!(client.graphql(QUERY, json!({})).unwrap().data["viewer"]["login"], "ada");
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(11)], "the ten seconds to the reset, and one more");
}

#[test]
fn a_secondary_rate_limit_waits_as_long_as_retry_after_says() {
    let fixtures = Fixtures::new()
        .status("Q", 403, &[("retry-after", "7")], json!({"message": "You have exceeded a secondary rate limit"}).to_string())
        .ok("Q", data("ada"));
    let (client, waits) = client(&fixtures);
    client.graphql(QUERY, json!({})).unwrap();
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(7)]);
}

#[test]
fn a_wait_longer_than_a_minute_is_left_to_the_caller() {
    let fixtures = Fixtures::new().status("Q", 429, &[("retry-after", "600")], "{}");
    let (client, waits) = client(&fixtures);
    let error = client.graphql(QUERY, json!({})).err().unwrap();
    assert_eq!(error, ForgeError::RateLimited { retry_after: Some(600) });
    assert!(waits.lock().unwrap().is_empty(), "it did not sleep ten minutes");
    assert_eq!(fixtures.sent().len(), 1);
}

#[test]
fn a_forge_that_stays_limited_gives_up_after_four_tries() {
    let fixtures = Fixtures::new().status("Q", 429, &[("retry-after", "1")], "{}");
    let (client, waits) = client(&fixtures);
    let error = client.graphql(QUERY, json!({})).err().unwrap();
    assert_eq!(error, ForgeError::RateLimited { retry_after: Some(1) });
    assert_eq!(fixtures.sent().len(), 4);
    assert_eq!(waits.lock().unwrap().len(), 3);
}

#[test]
fn a_query_that_meets_a_server_error_is_tried_again_with_growing_pauses() {
    let fixtures = Fixtures::new().status("Q", 502, &[], "<html>Bad gateway</html>").status("Q", 503, &[], "").ok("Q", data("ada"));
    let (client, waits) = client(&fixtures);
    assert!(client.graphql(QUERY, json!({})).is_ok());
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(1), Duration::from_secs(2)]);
}

#[test]
fn a_mutation_that_meets_a_server_error_is_never_sent_twice() {
    let fixtures = Fixtures::new().status("M", 502, &[], "<html>Bad gateway</html>").ok("M", "{}");
    let (client, _) = client(&fixtures);
    let error = client.graphql(MUTATION, json!({"input": {}})).err().unwrap();
    assert!(matches!(error, ForgeError::Unexpected(ref text) if text.contains("502")), "{error:?}");
    assert_eq!(fixtures.sent().len(), 1);
}

#[test]
fn a_rest_write_is_not_retried_on_a_server_error_but_a_read_is() {
    let fixtures = Fixtures::new().status("POST-repos-o-r-x", 500, &[], "{}").status("GET-repos-o-r-x", 500, &[], "{}").ok("GET-repos-o-r-x", "{}");
    let (client, _) = client(&fixtures);
    assert!(client.rest("POST", "repos/o/r/x", Some(json!({}))).is_err());
    assert_eq!(fixtures.sent_for("POST-repos-o-r-x").len(), 1);
    assert!(client.rest("GET", "repos/o/r/x", None).is_ok());
    assert_eq!(fixtures.sent_for("GET-repos-o-r-x").len(), 2);
}

#[test]
fn statuses_become_errors_the_ui_can_show() {
    let status = |code, body: &str| {
        let fixtures = Fixtures::new().status("GET-repos-o-r-x", code, &[], body);
        client(&fixtures).0.rest("GET", "repos/o/r/x", None).err().unwrap()
    };
    assert_eq!(status(401, r#"{"message":"Bad credentials"}"#), ForgeError::NotSignedIn);
    assert_eq!(status(403, r#"{"message":"Resource not accessible"}"#), ForgeError::Denied("Resource not accessible".into()));
    assert_eq!(status(404, "{}"), ForgeError::NotFound("repos/o/r/x".into()));
    assert_eq!(status(422, r#"{"message":"Validation Failed"}"#), ForgeError::Rejected("Validation Failed".into()));
    assert_eq!(status(409, "not json"), ForgeError::Rejected("HTTP 409".into()));
}

#[test]
fn transport_failures_map_to_their_own_errors() {
    let failing = |error| client(&Fixtures::new().fails("Q", error)).0.graphql(QUERY, json!({})).err().unwrap();
    assert_eq!(failing(TransportError::ToolMissing), ForgeError::ToolMissing { tool: "gh".into() });
    assert_eq!(failing(TransportError::NotSignedIn), ForgeError::NotSignedIn);
    assert_eq!(failing(TransportError::Offline), ForgeError::Offline);
}

#[test]
fn graphql_errors_beside_data_are_kept_and_errors_without_data_fail() {
    let partial = json!({"data": {"a": null}, "errors": [{"type": "NOT_FOUND", "message": "no such pull"}]}).to_string();
    let graph = client(&Fixtures::new().ok("Q", partial)).0.graphql(QUERY, json!({})).unwrap();
    assert_eq!(graph.errors.len(), 1);
    assert_eq!(graph.whole().err().unwrap(), ForgeError::NotFound("no such pull".into()));

    let none = json!({"data": null, "errors": [{"type": "FORBIDDEN", "message": "no access"}]}).to_string();
    let error = client(&Fixtures::new().ok("Q", none)).0.graphql(QUERY, json!({})).err().unwrap();
    assert_eq!(error, ForgeError::Denied("no access".into()));
}

#[test]
fn an_answer_that_is_not_json_is_unexpected_not_a_panic() {
    let error = client(&Fixtures::new().ok("Q", "<html>")).0.graphql(QUERY, json!({})).err().unwrap();
    assert!(matches!(error, ForgeError::Unexpected(_)));
}

#[test]
fn pages_follow_the_cursor_until_the_last_and_stop_at_the_cap() {
    let page = |items: &[u32], next: Option<&str>| {
        json!({"data": {"n": items, "next": next}}).to_string()
    };
    let fixtures = Fixtures::new().ok("Q", page(&[1, 2], Some("c1"))).ok("Q", page(&[3], Some("c2"))).ok("Q", page(&[4], None));
    let (client, _) = client(&fixtures);
    let read = |data: serde_json::Value| {
        let items: Vec<u32> = serde_json::from_value(data["n"].clone()).unwrap();
        Ok((items, data["next"].as_str().map(str::to_string)))
    };
    assert_eq!(client.pages(QUERY, json!({}), read).unwrap(), [1, 2, 3, 4]);
    let sent = fixtures.sent();
    assert!(sent[0].body.as_deref().unwrap().contains("\"variables\":{}"));
    assert!(sent[1].body.as_deref().unwrap().contains("\"after\":\"c1\""));
    assert!(sent[2].body.as_deref().unwrap().contains("\"after\":\"c2\""));

    let endless = Fixtures::new().ok("Q", page(&[1], Some("again")));
    let error = client_for(&endless).pages(QUERY, json!({}), read).err().unwrap();
    assert!(matches!(error, ForgeError::Unexpected(text) if text.contains("500 pages")));
}

fn client_for(fixtures: &Fixtures) -> Client {
    client(fixtures).0
}
