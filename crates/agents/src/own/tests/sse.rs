use crate::own::sse::{Parser, SseEvent};

fn parse(chunks: &[&[u8]]) -> Vec<SseEvent> {
    let mut parser = Parser::default();
    let mut out = Vec::new();
    for chunk in chunks {
        parser.feed(chunk, &mut |e| out.push(e)).unwrap();
    }
    parser.finish(&mut |e| out.push(e));
    out
}

fn ev(name: Option<&str>, data: &str) -> SseEvent {
    SseEvent { name: name.map(str::to_string), data: data.to_string() }
}

const STREAM: &[u8] = b"event: message_start\ndata: {\"a\":1}\n\n: a comment\nevent: ping\ndata: {\"b\":2}\n\ndata: line one\ndata: line two\n\n";

#[test]
fn events_have_a_name_and_data_and_comments_are_skipped() {
    assert_eq!(parse(&[STREAM]), vec![ev(Some("message_start"), "{\"a\":1}"), ev(Some("ping"), "{\"b\":2}"), ev(None, "line one\nline two")]);
}

#[test]
fn the_result_is_the_same_however_the_bytes_are_cut() {
    let whole = parse(&[STREAM]);
    for size in 1..=STREAM.len() {
        let chunks: Vec<&[u8]> = STREAM.chunks(size).collect();
        assert_eq!(parse(&chunks), whole, "cut every {size} bytes");
    }
}

#[test]
fn a_character_split_across_two_pieces_is_whole_in_the_event() {
    let text = "data: h\u{e9}llo \u{1f600}\n\n".as_bytes();
    for cut in 1..text.len() {
        assert_eq!(parse(&[&text[..cut], &text[cut..]]), vec![ev(None, "h\u{e9}llo \u{1f600}")], "cut at {cut}");
    }
}

#[test]
fn line_ends_may_be_crlf_and_a_space_after_the_colon_is_optional() {
    assert_eq!(parse(&[b"event: x\r\ndata:{\"k\":1}\r\n\r\ndata: y\r\n\r\n"]), vec![ev(Some("x"), "{\"k\":1}"), ev(None, "y")]);
}

#[test]
fn a_last_event_with_no_blank_line_after_it_still_arrives_and_a_name_alone_is_no_event() {
    assert_eq!(parse(&[b"data: last"]), vec![ev(None, "last")]);
    assert_eq!(parse(&[b"event: lonely\n\ndata: next\n\n"]), vec![ev(None, "next")], "the name does not leak into the next event");
}

#[test]
fn a_line_that_never_ends_is_refused_instead_of_kept_forever() {
    let mut parser = Parser::default();
    let big = vec![b'a'; 1024 * 1024];
    let mut failed = false;
    for _ in 0..20 {
        if parser.feed(&big, &mut |_| {}).is_err() {
            failed = true;
            break;
        }
    }
    assert!(failed);
}
