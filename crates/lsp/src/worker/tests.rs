use super::*;
use lsp_types::{Location, Range, Uri};

fn uri(path: &str) -> Uri {
    path.parse().expect("a valid uri")
}

fn range(line: u32, from: u32, to: u32) -> Range {
    Range { start: Position { line, character: from }, end: Position { line, character: to } }
}

#[test]
fn text_the_server_already_has_is_not_sent_again() {
    let document = DocumentSync::opened("fn a() {}");
    assert_eq!(document.next("fn a() {}"), None);
    assert_eq!(document.version(), 1);
}

#[test]
fn each_new_text_goes_out_as_the_next_version_once_it_is_sent() {
    let mut document = DocumentSync::opened("a");
    assert_eq!(document.next("ab"), Some(2));
    assert_eq!(document.next("ab"), Some(2), "nothing is recorded until the send worked");
    document.sent("ab", 2);
    assert_eq!(document.next("ab"), None, "the same text twice is one change");
    assert_eq!(document.next("a"), Some(3), "going back is still a change");
    assert_eq!(document.version(), 2);
}

#[test]
fn no_answer_is_no_links() {
    assert!(definition_links(None).is_empty());
    assert_eq!(first_line(&[]), None);
}

#[test]
fn a_plain_location_becomes_a_link_that_selects_its_range() {
    let location = Location { uri: uri("file:///a.rs"), range: range(4, 7, 12) };
    let links = definition_links(Some(GotoDefinitionResponse::Scalar(location)));
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_selection_range, range(4, 7, 12));
    assert_eq!(links[0].origin_selection_range, None, "we never invent which word was clicked");
    assert_eq!(first_line(&links), Some(4));
}

#[test]
fn every_location_in_a_list_is_kept_in_order() {
    let locations = vec![
        Location { uri: uri("file:///a.rs"), range: range(1, 0, 1) },
        Location { uri: uri("file:///b.rs"), range: range(9, 0, 1) },
    ];
    let links = definition_links(Some(GotoDefinitionResponse::Array(locations)));
    assert_eq!(links.iter().map(|l| l.target_uri.as_str()).collect::<Vec<_>>(), ["file:///a.rs", "file:///b.rs"]);
}

#[test]
fn a_link_answer_keeps_its_own_name_range() {
    let link = LocationLink {
        origin_selection_range: Some(range(6, 4, 9)),
        target_uri: uri("file:///a.rs"),
        target_range: range(0, 0, 30),
        target_selection_range: range(0, 7, 12),
    };
    let links = definition_links(Some(GotoDefinitionResponse::Link(vec![link.clone()])));
    assert_eq!(links, vec![link]);
    assert_eq!(first_line(&links), Some(0));
}

fn modified() -> LspError {
    LspError::Server { code: CONTENT_MODIFIED, message: "content modified".into() }
}

#[test]
fn a_request_the_text_moved_under_is_asked_again() {
    let mut calls = 0;
    let answer = until_settled(Duration::from_secs(5), || {
        calls += 1;
        if calls < 3 { Err(modified()) } else { Ok(calls) }
    });
    assert_eq!(answer.expect("the third try answers"), 3);
}

#[test]
fn any_other_error_comes_back_at_once() {
    let mut calls = 0;
    let answer: Result<(), _> = until_settled(Duration::from_secs(5), || {
        calls += 1;
        Err(LspError::Timeout)
    });
    assert!(matches!(answer, Err(LspError::Timeout)));
    assert_eq!(calls, 1, "only a moved text is worth asking again");
}

#[test]
fn a_server_that_never_settles_gives_up_by_the_deadline() {
    let started = std::time::Instant::now();
    let answer: Result<(), _> = until_settled(Duration::from_millis(300), || Err(modified()));
    assert!(matches!(answer, Err(LspError::Server { code: CONTENT_MODIFIED, .. })));
    assert!(started.elapsed() < Duration::from_millis(600), "it stops near the deadline");
}

#[test]
fn a_request_the_server_cancelled_is_asked_again() {
    let mut calls = 0;
    let answer = until_settled(Duration::from_secs(5), || {
        calls += 1;
        if calls < 2 { Err(LspError::Server { code: SERVER_CANCELLED, message: "busy".into() }) } else { Ok(()) }
    });
    assert!(answer.is_ok());
    assert_eq!(calls, 2);
}

#[test]
fn only_the_newest_diagnostics_request_in_a_batch_runs() {
    let (tx, rx) = std::sync::mpsc::channel();
    let check = |name: &'static str| {
        let tx = tx.clone();
        Job::Diagnostics { text: name.into(), reply: Box::new(move |a| { let _ = tx.send((name, a.is_err())); }) }
    };
    let hover = Job::Hover { text: "h".into(), position: Position::default(), reply: Box::new(|_| {}) };
    let jobs = triage(vec![check("old"), hover, check("older"), check("newest")]);
    let kept: Vec<_> = jobs
        .iter()
        .map(|job| match job {
            Job::Diagnostics { text, .. } => text.as_str(),
            Job::Hover { .. } => "hover",
            Job::Definition { .. } => "definition",
        })
        .collect();
    assert_eq!(kept, ["hover", "newest"], "other requests keep their order");
    // The kept jobs hold senders too, so they go before the answers are read.
    drop((jobs, tx));
    let told: Vec<_> = rx.iter().collect();
    assert_eq!(told, [("old", true), ("older", true)], "each dropped request hears it was superseded");
}
