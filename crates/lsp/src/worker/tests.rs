use super::*;

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
fn only_the_newest_diagnostics_request_for_each_document_runs() {
    let (tx, rx) = std::sync::mpsc::channel();
    let check = |path: &'static str, name: &'static str| {
        let tx = tx.clone();
        let doc = Doc { path: path.into(), text: name.into() };
        Job::Diagnostics { doc, reply: Box::new(move |a| { let _ = tx.send((name, a.is_err())); }) }
    };
    let doc = Doc { path: "a.rs".into(), text: "h".into() };
    let hover = Job::Hover { doc, position: Position::default(), reply: Box::new(|_| {}) };
    let jobs = triage(vec![check("a.rs", "old"), hover, check("b.rs", "other file"), check("a.rs", "newest")]);
    let kept: Vec<_> = jobs
        .iter()
        .map(|job| match job {
            Job::Diagnostics { doc, .. } => doc.text.as_str(),
            Job::Hover { .. } => "hover",
            Job::Navigate { .. } | Job::References { .. } => "other",
        })
        .collect();
    assert_eq!(kept, ["hover", "other file", "newest"], "other requests keep their order");
    // The kept jobs hold senders too, so they go before the answers are read.
    drop((jobs, tx));
    let told: Vec<_> = rx.iter().collect();
    assert_eq!(told, [("old", true)], "only the older check for the same file is superseded");
}

#[test]
fn a_file_uri_names_its_path_again() {
    let path = std::env::temp_dir().join("lathe uri test/a%b.rs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();
    let uri = path_to_uri(&path).expect("an existing path has a uri");
    assert_eq!(uri_to_path(&uri), Some(path.clone()));
    assert_eq!(uri_to_path(&"https://docs.rs/x".parse().unwrap()), None, "not a file");
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}
