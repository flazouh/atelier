use super::{ReadError, Request, Response, read_request, write_response};

fn read(text: &str) -> Result<Request, ReadError> {
    read_request(&mut text.as_bytes())
}

#[test]
fn a_post_with_a_body_is_read_whole() {
    let request =
        read("POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nX-Thing:  a b \r\n\r\nhello")
            .unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/mcp");
    assert_eq!(request.body, b"hello");
    assert_eq!(request.header("x-thing"), Some("a b"));
}

#[test]
fn a_body_shorter_than_its_length_is_gone() {
    assert_eq!(
        read("POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\nshort").unwrap_err(),
        ReadError::Gone
    );
}

#[test]
fn a_body_over_the_limit_is_too_large() {
    assert_eq!(
        read("POST / HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n").unwrap_err(),
        ReadError::TooLarge
    );
}

#[test]
fn chunks_are_refused() {
    assert_eq!(
        read("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap_err(),
        ReadError::Chunked
    );
}

#[test]
fn garbage_is_malformed() {
    assert_eq!(read("hello\r\n\r\n").unwrap_err(), ReadError::Malformed);
    assert_eq!(
        read("GET / HTTP/1.1\r\nno colon here\r\n\r\n").unwrap_err(),
        ReadError::Malformed
    );
    assert_eq!(
        read("GET / HTTP/1.1\r\nContent-Length: x\r\n\r\n").unwrap_err(),
        ReadError::Malformed
    );
}

#[test]
fn a_head_that_never_ends_is_too_large() {
    let long = format!("GET / HTTP/1.1\r\nX: {}", "a".repeat(40_000));
    assert_eq!(read(&long).unwrap_err(), ReadError::TooLarge);
}

#[test]
fn an_answer_has_a_length_and_closes() {
    let mut out = Vec::new();
    write_response(&mut out, &Response::json(200, &serde_json::json!({"a": 1}))).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text.contains("Content-Length: 7\r\n"));
    assert!(text.contains("Connection: close\r\n"));
    assert!(text.ends_with("\r\n\r\n{\"a\":1}"));
}
