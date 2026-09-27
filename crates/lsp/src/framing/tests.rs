use std::io::Cursor;

use super::*;

fn framed(body: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_message(&mut out, body.as_bytes()).expect("writing to a vec cannot fail");
    out
}

#[test]
fn a_message_carries_its_byte_length_not_its_character_count() {
    // "é" is two bytes, so a character count would frame it short and desynchronise the stream.
    let body = "{\"t\":\"é\"}";
    assert_eq!(body.chars().count(), 9, "nine characters");
    assert_eq!(body.len(), 10, "ten bytes");
    let out = framed(body);
    let header = String::from_utf8_lossy(&out[..out.iter().position(|b| *b == b'\r').unwrap()]).to_string();
    assert_eq!(header, "Content-Length: 10");
}

#[test]
fn a_written_message_reads_back_whole() {
    let mut input = Cursor::new(framed("{\"id\":1}"));
    let body = read_message(&mut input).expect("it reads").expect("it is not the end");
    assert_eq!(body, b"{\"id\":1}");
}

#[test]
fn two_messages_in_one_stream_read_in_order() {
    let mut bytes = framed("{\"id\":1}");
    bytes.extend(framed("{\"id\":2}"));
    let mut input = Cursor::new(bytes);
    assert_eq!(read_message(&mut input).unwrap().unwrap(), b"{\"id\":1}");
    assert_eq!(read_message(&mut input).unwrap().unwrap(), b"{\"id\":2}");
    assert!(read_message(&mut input).unwrap().is_none(), "the stream ends cleanly");
}

#[test]
fn a_header_we_do_not_know_is_skipped() {
    let body = "{\"ok\":true}";
    let raw = format!("Content-Type: application/vscode-jsonrpc\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let mut input = Cursor::new(raw.into_bytes());
    assert_eq!(read_message(&mut input).unwrap().unwrap(), body.as_bytes());
}

#[test]
fn a_message_with_no_length_header_is_an_error() {
    let mut input = Cursor::new(b"Content-Type: x\r\n\r\n{}".to_vec());
    assert!(read_message(&mut input).is_err());
}

#[test]
fn an_empty_stream_is_the_end_not_an_error() {
    let mut input = Cursor::new(Vec::new());
    assert!(read_message(&mut input).unwrap().is_none());
}
