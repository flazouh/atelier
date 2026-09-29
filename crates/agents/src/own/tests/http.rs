use std::{
    io::Read,
    time::{Duration, Instant},
};

use super::server::{FakeServer, Step, says};
use crate::own::{
    http::{HttpError, HttpOptions, HttpRequest, send},
    message::Cancel,
};

fn get(url: &str) -> HttpRequest {
    HttpRequest { method: "POST", url: url.to_string(), headers: vec![("x-test".into(), "yes".into())], body: b"{\"a\":1}".to_vec() }
}

fn body_of(server: &FakeServer, chunk: usize, bytes: Vec<u8>) -> Vec<u8> {
    let _ = server;
    let server = FakeServer::start(vec![Step::Raw { bytes, chunk }]);
    let mut response = send(&get(&format!("{}/x", server.url)), &Cancel::default(), &HttpOptions::default()).unwrap();
    assert_eq!(response.status, 200);
    let mut out = Vec::new();
    response.body.read_to_end(&mut out).unwrap();
    out
}

#[test]
fn a_chunked_body_reads_whole_however_it_was_cut() {
    let bytes: Vec<u8> = (0..5000u32).map(|i| b'a' + (i % 26) as u8).collect();
    let anchor = FakeServer::start(vec![]);
    for chunk in [1, 2, 7, 64, 4999, 100_000] {
        assert_eq!(body_of(&anchor, chunk, bytes.clone()), bytes, "chunks of {chunk}");
    }
}

#[test]
fn the_request_has_a_host_a_length_the_headers_given_and_the_body() {
    let server = FakeServer::start(vec![Step::Status { code: 200, headers: vec![], body: "{}".into() }]);
    let mut response = send(&get(&format!("{}/path?q=1", server.url)), &Cancel::default(), &HttpOptions::default()).unwrap();
    let mut text = String::new();
    response.body.read_to_string(&mut text).unwrap();
    assert_eq!(text, "{}", "a body with a length");
    let request = &server.recorded()[0];
    assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/path?q=1"));
    assert_eq!(request.header("x-test"), Some("yes"));
    assert_eq!(request.header("content-length"), Some("7"));
    assert!(request.header("host").unwrap().starts_with("127.0.0.1:"));
    assert_eq!(request.body["a"], 1);
}

#[test]
fn a_status_and_its_headers_are_read_before_the_body() {
    let server = FakeServer::start(vec![Step::Status { code: 429, headers: vec![("Retry-After", "7".into())], body: "slow".into() }]);
    let mut response = send(&get(&server.url), &Cancel::default(), &HttpOptions::default()).unwrap();
    assert_eq!(response.status, 429);
    assert_eq!(response.header("retry-after"), Some("7"), "header names ignore case");
    assert_eq!(response.body.text(100), "slow");
}

#[test]
fn a_url_that_is_not_usable_is_refused_without_a_connection() {
    for url in ["ftp://x/y", "no-scheme", "http://", "http://user:pw@host/", "http://127.0.0.1:notaport/"] {
        let error = send(&get(url), &Cancel::default(), &HttpOptions::default()).err().unwrap_or_else(|| panic!("{url} was accepted"));
        assert!(matches!(error, HttpError::Connect(_)), "{url}: {error:?}");
    }
}

#[test]
fn a_refused_connection_is_a_connect_error_that_names_the_host_and_no_header() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let mut request = get(&format!("http://127.0.0.1:{port}/"));
    request.headers.push(("x-api-key".into(), "sk-secret-value".into()));
    let error = send(&request, &Cancel::default(), &HttpOptions::default()).err().unwrap();
    let HttpError::Connect(why) = &error else { panic!("{error:?}") };
    assert!(why.contains("127.0.0.1") && !why.contains("sk-secret-value"), "{why}");
}

#[test]
fn a_header_with_a_line_break_cannot_be_sent() {
    let mut request = get("http://127.0.0.1:1/");
    request.headers.push(("x-evil".into(), "a\r\nInjected: 1".into()));
    let server = FakeServer::start(vec![]);
    request.url = server.url.clone();
    assert!(matches!(send(&request, &Cancel::default(), &HttpOptions::default()), Err(HttpError::Connect(_))));
    assert!(server.recorded().is_empty() || server.recorded()[0].header("injected").is_none());
}

#[test]
fn a_request_stopped_before_it_starts_makes_no_connection() {
    let server = FakeServer::start(vec![Step::Sse(says("x"))]);
    let cancel = Cancel::default();
    cancel.set();
    assert_eq!(send(&get(&server.url), &cancel, &HttpOptions::default()).err(), Some(HttpError::Cancelled));
    std::thread::sleep(Duration::from_millis(50));
    assert!(server.recorded().is_empty());
}

#[test]
fn a_read_that_waits_stops_soon_after_the_flag_is_set_and_closes_the_connection() {
    let server = FakeServer::start(vec![Step::Hang(vec![])]);
    let cancel = Cancel::default();
    let mut response = send(&get(&server.url), &cancel, &HttpOptions::default()).unwrap();
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        flag.set();
    });
    let started = Instant::now();
    let mut buf = [0u8; 16];
    let error = response.body.read(&mut buf).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    drop(response);
    let limit = Instant::now() + Duration::from_secs(5);
    while !server.hung_up.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(server.hung_up.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn a_server_that_goes_silent_is_given_up_on_after_the_idle_time() {
    let server = FakeServer::start(vec![Step::Hang(vec![])]);
    let options = HttpOptions { idle_timeout: Duration::from_millis(300), ..HttpOptions::default() };
    let mut response = send(&get(&server.url), &Cancel::default(), &options).unwrap();
    let started = Instant::now();
    let mut buf = [0u8; 16];
    let error = response.body.read(&mut buf).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(started.elapsed() >= Duration::from_millis(250) && started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_body_that_ends_early_is_an_error_not_a_short_body() {
    // A chunk that says 100 bytes and sends 3, then the connection ends.
    let server = FakeServer::start(vec![Step::Raw { bytes: b"abc".to_vec(), chunk: 3 }]);
    let mut response = send(&get(&server.url), &Cancel::default(), &HttpOptions::default()).unwrap();
    let mut out = Vec::new();
    // The fake ends the body properly, so this reads "abc"; the broken case is the length one below.
    response.body.read_to_end(&mut out).unwrap();
    assert_eq!(out, b"abc");

    let short = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", short.local_addr().unwrap());
    std::thread::spawn(move || {
        use std::io::{Read as _, Write as _};
        let (mut stream, _) = short.accept().unwrap();
        let mut sink = [0u8; 4096];
        let _ = stream.read(&mut sink);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nabc");
    });
    let mut response = send(&get(&url), &Cancel::default(), &HttpOptions::default()).unwrap();
    let error = response.body.read_to_end(&mut Vec::new()).unwrap_err();
    // The server hung up mid-body: the end of the stream, or a reset when it closed with our bytes unread.
    assert!(matches!(error.kind(), std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset), "{error:?}");
}
