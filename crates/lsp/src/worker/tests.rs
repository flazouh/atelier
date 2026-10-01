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
fn only_the_newest_question_of_each_kind_per_document_runs() {
    let (tx, rx) = std::sync::mpsc::channel();
    let doc = |path: &str, text: &str| Doc { path: path.into(), text: text.into() };
    let told = |name: &'static str| {
        let tx = tx.clone();
        move |failed: bool| {
            let _ = tx.send((name, failed));
        }
    };
    let check = |path: &str, name: &'static str| {
        let tell = told(name);
        Job::Diagnostics { doc: doc(path, name), reply: Box::new(move |a| tell(a.is_err())) }
    };
    let hover = |path: &str, name: &'static str| {
        let tell = told(name);
        Job::Hover { doc: doc(path, name), position: Position::default(), reply: Box::new(move |a| tell(a.is_err())) }
    };
    let find = |path: &str, name: &'static str| {
        let tell = told(name);
        Job::References { doc: doc(path, name), position: Position::default(), reply: Box::new(move |a| tell(a.is_err())) }
    };
    let names = |path: &str, name: &'static str| {
        let tell = told(name);
        Job::ProjectSymbols { doc: doc(path, name), query: name.into(), reply: Box::new(move |a| tell(a.is_err())) }
    };
    let batch = vec![
        names("a.rs", "old query"),
        check("a.rs", "old check"),
        hover("a.rs", "old hover"),
        find("a.rs", "first find"),
        check("b.rs", "other file"),
        hover("a.rs", "new hover"),
        find("a.rs", "second find"),
        check("a.rs", "new check"),
        names("b.rs", "new query"),
    ];
    let jobs = triage(batch);
    let kept: Vec<_> = jobs
        .iter()
        .map(|job| match job {
            Job::Diagnostics { doc, .. }
            | Job::Hover { doc, .. }
            | Job::References { doc, .. }
            | Job::Symbols { doc, .. }
            | Job::ProjectSymbols { doc, .. } => doc.text.clone(),
            Job::Navigate { .. } => "navigate".into(),
        })
        .collect();
    assert_eq!(kept, ["first find", "other file", "new hover", "second find", "new check", "new query"], "order is kept");
    // The kept jobs hold senders too, so they go before the answers are read.
    drop((jobs, tx));
    let mut told: Vec<_> = rx.iter().collect();
    told.sort();
    assert_eq!(
        told,
        [("old check", true), ("old hover", true), ("old query", true)],
        "each dropped question hears it was superseded, a project query whatever file it came from"
    );
}

#[test]
fn a_path_is_canonical_and_a_missing_file_keeps_its_path() {
    let dir = std::env::temp_dir().join(format!("atelier-canonical-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::write(dir.join("a/x.rs"), "").unwrap();
    assert_eq!(canonical(&dir.join("a/../a/x.rs")), std::fs::canonicalize(dir.join("a/x.rs")).unwrap());
    assert_eq!(canonical(Path::new("/no/such/file.rs")), PathBuf::from("/no/such/file.rs"));
    std::fs::remove_dir_all(&dir).ok();
}
