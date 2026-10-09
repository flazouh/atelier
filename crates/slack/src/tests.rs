use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use atelier_capabilities::{
    Actor, CapError, Ref,
    messaging::{
        AttachmentKind, ChannelKind, ChannelQuery, EventKind, Filter, MessagingProvider,
        NewMessage, Operation, SearchQuery, contract, contract::Seed,
    },
};

use crate::{
    ChannelSpec, CommandRunner, Output, RunError, Runner, SlackConfig, SlackMessaging,
    fake::FakeSlack, parse_permalink, to_mrkdwn,
};

const WHOAMI: &str = include_str!("../tests/fixtures/whoami.json");
const READ: &str = include_str!("../tests/fixtures/read.json");
const THREAD: &str = include_str!("../tests/fixtures/thread.json");
const SEARCH: &str = include_str!("../tests/fixtures/search.json");
const POSTED: &str = include_str!("../tests/fixtures/posted.json");
const USERS: &str = include_str!("../tests/fixtures/users.json");

/// Answers each command from a fixture and remembers what it was asked.
#[derive(Clone, Default)]
struct Fixtures {
    answers: Arc<Mutex<Vec<(String, Output)>>>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl Fixtures {
    fn with(self, command: &str, output: Output) -> Self {
        self.answers.lock().unwrap().push((command.into(), output));
        self
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    fn the_usual() -> Self {
        Self::default()
            .with("whoami", Output::ok(WHOAMI))
            .with("read", Output::ok(READ))
            .with("thread", Output::ok(THREAD))
            .with("search", Output::ok(SEARCH))
            .with("send", Output::ok(POSTED))
            .with("reply", Output::ok(POSTED))
            .with("edit", Output::ok(POSTED))
            .with("users", Output::ok(USERS))
    }
}

impl Runner for Fixtures {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        self.calls.lock().unwrap().push(args.to_vec());
        let answers = self.answers.lock().unwrap();
        Ok(answers
            .iter()
            .find(|(c, _)| c == &args[0])
            .map(|(_, o)| o.clone())
            .unwrap_or_else(|| Output::failed("Unknown command")))
    }
}

fn config() -> SlackConfig {
    SlackConfig::new(
        "acme",
        vec![
            ChannelSpec::new("C01GENERAL", "general", ChannelKind::Public),
            ChannelSpec::new("C02RANDOM", "random", ChannelKind::Public),
            ChannelSpec::new("D01SAM", "sam", ChannelKind::Dm),
        ],
    )
}

fn provider(fixtures: &Fixtures) -> SlackMessaging {
    SlackMessaging::new(fixtures.clone(), config())
}

fn general() -> Ref {
    "messaging:slack:acme:C01GENERAL".parse().unwrap()
}

#[test]
fn slack_passes_the_contract_on_a_fake_slack() {
    contract::run(&|| {
        let mut config = SlackConfig::new(
            "acme",
            vec![
                ChannelSpec::new("C01", "general", ChannelKind::Public),
                ChannelSpec::new("D01", "sam", ChannelKind::Dm),
            ],
        );
        config.poll_every = Duration::from_millis(20);
        Seed {
            provider: Box::new(SlackMessaging::new(FakeSlack::new(), config)),
            public: "messaging:slack:acme:C01".parse().unwrap(),
            dm: "messaging:slack:acme:D01".parse().unwrap(),
        }
    });
}

#[test]
fn history_maps_rows_newest_first_with_files_and_reply_counts() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures).history(&general(), None, None).unwrap();
    let texts: Vec<&str> = page.items.iter().map(|m| m.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "[file] shot.png (image/png, 412 KB)",
            "Who can review the release notes?",
            "Morning. The build is green."
        ]
    );
    let with_replies = &page.items[1];
    assert_eq!(with_replies.reply_count, 2);
    assert_eq!(
        with_replies.reference.to_string(),
        "messaging:slack:acme:C01GENERAL:1760000080.000200"
    );
    assert_eq!(with_replies.channel, general());
    assert_eq!(with_replies.created_at, 1_760_000_080_000);
    assert_eq!(with_replies.author.id, "U01ALEX", "my own post is me");
    assert_eq!(
        page.items[2].author.id, "sam",
        "another author has the name as id"
    );
    let files = &page.items[0].attachments;
    assert_eq!(files.len(), 2);
    assert_eq!(
        (files[0].kind, files[0].name.as_str()),
        (AttachmentKind::Image, "shot.png")
    );
    assert_eq!(files[0].size, Some(421_888));
    assert_eq!(
        files[1].name, "Notes",
        "a file with no name takes its title"
    );
    assert!(page.next_cursor.is_none());
    assert!(page.items[0].raw.is_some(), "the row is kept");
}

#[test]
fn history_asks_one_more_row_than_it_shows_to_know_there_is_more() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures)
        .history(&general(), Some("1"), Some(1))
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].text, "Who can review the release notes?");
    assert_eq!(page.next_cursor.as_deref(), Some("2"));
    let read = fixtures
        .calls()
        .into_iter()
        .find(|c| c[0] == "read")
        .unwrap();
    assert_eq!(read, ["read", "--json", "--full", "-n", "3", "C01GENERAL"]);
}

#[test]
fn a_thread_has_the_root_first_and_replies_that_name_it() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures)
        .thread(
            &"messaging:slack:acme:C01GENERAL:1760000110.000400"
                .parse()
                .unwrap(),
            None,
        )
        .unwrap();
    let root = "messaging:slack:acme:C01GENERAL:1760000080.000200";
    assert_eq!(page.items.len(), 3);
    assert!(page.items[0].parent.is_none());
    assert_eq!(page.items[0].reference.to_string(), root);
    for reply in &page.items[1..] {
        assert_eq!(
            reply.parent.as_ref().map(ToString::to_string).as_deref(),
            Some(root)
        );
        assert_eq!(reply.reply_count, 0);
    }
    assert_eq!(page.items[2].text, "Thanks.");
}

#[test]
fn search_maps_the_channel_from_the_link_and_skips_a_row_with_none() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures)
        .search(&SearchQuery {
            text: "release".into(),
            channel: Some(general()),
            from: Some("sam".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(page.items.len(), 2, "the row with no link is left out");
    assert_eq!(
        page.items[1].channel.to_string(),
        "messaging:slack:acme:C02RANDOM"
    );
    assert_eq!(
        page.items[1].reference.to_string(),
        "messaging:slack:acme:C02RANDOM:1759917600.000700",
        "an archives link reads like an app link"
    );
    let search = fixtures
        .calls()
        .into_iter()
        .find(|c| c[0] == "search")
        .unwrap();
    assert_eq!(
        search,
        [
            "search",
            "--json",
            "--full",
            "-n",
            "21",
            "--",
            "release in:general from:@sam"
        ]
    );
}

#[test]
fn search_in_a_channel_the_app_does_not_know_is_invalid() {
    let fixtures = Fixtures::the_usual();
    let unknown: Ref = "messaging:slack:acme:C09NOPE".parse().unwrap();
    let result = provider(&fixtures).search(&SearchQuery {
        text: "x".into(),
        channel: Some(unknown),
        ..SearchQuery::default()
    });
    assert!(matches!(result, Err(CapError::Invalid { .. })));
}

#[test]
fn send_gives_the_text_after_the_flags_so_a_leading_dash_is_not_a_flag() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let me = p.whoami().unwrap();
    let sent = p
        .send(
            &NewMessage::to(&general(), "-1 agreed, see **bold** & <!channel>"),
            &me,
        )
        .unwrap();
    let call = fixtures
        .calls()
        .into_iter()
        .find(|c| c[0] == "send")
        .unwrap();
    assert_eq!(
        call,
        [
            "send",
            "--yes",
            "--json",
            "C01GENERAL",
            "--",
            "-1 agreed, see *bold* &amp; &lt;!channel&gt;"
        ]
    );
    assert_eq!(
        sent.text, "-1 agreed, see **bold** & <!channel>",
        "the text the caller wrote"
    );
    assert_eq!(
        sent.reference.to_string(),
        "messaging:slack:acme:C01GENERAL:1760000200.000900"
    );
    assert_eq!(sent.created_at, 1_760_000_200_000);
    assert!(sent.origin.is_none());
}

#[test]
fn an_agent_message_says_in_its_text_who_sent_it() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let agent = Actor::agent("agent-1", "Claude", "U01ALEX");
    let sent = p
        .send(&NewMessage::to(&general(), "On it."), &agent)
        .unwrap();
    assert_eq!(sent.origin.as_deref(), Some("alex's agent"));
    assert_eq!(sent.author, agent, "the actor is kept");
    let call = fixtures
        .calls()
        .into_iter()
        .find(|c| c[0] == "send")
        .unwrap();
    assert_eq!(call.last().unwrap(), "On it.\n\n_sent by alex's agent_");
}

#[test]
fn a_reply_goes_to_the_root_of_the_thread() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let me = p.whoami().unwrap();
    let to_a_reply: Ref = "messaging:slack:acme:C01GENERAL:1760000110.000400"
        .parse()
        .unwrap();
    let sent = p
        .send(&NewMessage::reply(&to_a_reply, &general(), "Done."), &me)
        .unwrap();
    let calls = fixtures.calls();
    assert!(calls.iter().any(|c| c[..]
        == [
            "thread",
            "--json",
            "--full",
            "-n",
            "1",
            "C01GENERAL:1760000110.000400"
        ]));
    let reply = calls.iter().find(|c| c[0] == "reply").unwrap();
    assert_eq!(
        reply[3], "C01GENERAL:1760000080.000200",
        "the root, not the reply"
    );
    assert_eq!(
        sent.parent.unwrap().to_string(),
        "messaging:slack:acme:C01GENERAL:1760000080.000200"
    );
}

#[test]
fn a_reply_must_stay_in_its_channel() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let me = p.whoami().unwrap();
    let other: Ref = "messaging:slack:acme:C02RANDOM".parse().unwrap();
    let root: Ref = "messaging:slack:acme:C01GENERAL:1760000080.000200"
        .parse()
        .unwrap();
    assert!(matches!(
        p.send(&NewMessage::reply(&root, &other, "x"), &me),
        Err(CapError::Invalid { .. })
    ));
    assert!(
        fixtures.calls().iter().all(|c| c[0] != "reply"),
        "nothing was sent"
    );
}

#[test]
fn edit_sets_the_edit_time_and_keeps_the_new_text() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let me = p.whoami().unwrap();
    let target: Ref = "messaging:slack:acme:C01GENERAL:1760000200.000900"
        .parse()
        .unwrap();
    let edited = p.edit(&target, "Better", &me).unwrap();
    assert_eq!(edited.text, "Better");
    assert!(edited.edited_at.is_some());
    let call = fixtures
        .calls()
        .into_iter()
        .find(|c| c[0] == "edit")
        .unwrap();
    assert_eq!(
        call,
        [
            "edit",
            "--yes",
            "--json",
            "C01GENERAL:1760000200.000900",
            "--",
            "Better"
        ]
    );
}

#[test]
fn a_person_is_found_by_slack_id_or_by_name() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let by_id = p
        .person(&"messaging:slack:acme:user/U01ALEX".parse().unwrap())
        .unwrap();
    assert_eq!(
        (by_id.name.as_str(), by_id.handle.as_str()),
        ("Alex Example", "alex")
    );
    assert_eq!(
        by_id.reference.to_string(),
        "messaging:slack:acme:user/U01ALEX"
    );
    let by_name = p
        .person(&"messaging:slack:acme:user/deploybot".parse().unwrap())
        .unwrap();
    assert!(by_name.is_bot);
    assert_eq!(
        by_name.reference.to_string(),
        "messaging:slack:acme:user/B01BOT"
    );
    assert!(matches!(
        p.person(&"messaging:slack:acme:user/nobody".parse().unwrap()),
        Err(CapError::NotFound { .. })
    ));
}

#[test]
fn channels_are_the_ones_the_app_listed() {
    let p = provider(&Fixtures::the_usual());
    let all = p.channels(&ChannelQuery::default()).unwrap();
    assert_eq!(all.items.len(), 3);
    let dms = p
        .channels(&ChannelQuery {
            kinds: vec![ChannelKind::Dm],
            ..ChannelQuery::default()
        })
        .unwrap();
    assert_eq!(
        dms.items[0].reference.to_string(),
        "messaging:slack:acme:D01SAM"
    );
    let workspace = p.workspace().unwrap();
    assert_eq!(workspace.name, "Acme");
    assert_eq!(
        workspace.reference.to_string(),
        "messaging:slack:acme:workspace"
    );
}

#[test]
fn what_slackcli_cannot_do_is_not_listed_and_never_runs() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let caps = p.capabilities();
    for op in [
        Operation::Delete,
        Operation::React,
        Operation::MarkRead,
        Operation::Export,
        Operation::Import,
    ] {
        assert!(!caps.can(op), "{op:?}");
    }
    let me = Actor::person("U01ALEX", "alex");
    let m: Ref = "messaging:slack:acme:C01GENERAL:1760000080.000200"
        .parse()
        .unwrap();
    assert!(matches!(
        p.delete(&m, &me),
        Err(CapError::Unsupported { .. })
    ));
    assert!(matches!(
        p.react(&m, "eyes", true, &me),
        Err(CapError::Unsupported { .. })
    ));
    assert!(matches!(
        p.mark_read(&general(), None),
        Err(CapError::Unsupported { .. })
    ));
    assert!(fixtures.calls().is_empty(), "no command ran");
}

#[test]
fn a_failed_command_is_the_error_the_screen_knows() {
    let case = |stderr: &str| {
        let fixtures = Fixtures::default().with("read", Output::failed(stderr));
        provider(&fixtures)
            .history(&general(), None, None)
            .unwrap_err()
    };
    assert_eq!(
        case(
            "No Slack credentials found (stored session, environment: no readable session). Set SLACK_TOKEN, or run: slackcli login"
        ),
        CapError::NotSignedIn
    );
    assert_eq!(
        case("Slack rejected the credential (invalid_auth). Run: slackcli login"),
        CapError::NotSignedIn
    );
    assert_eq!(
        case("Slack is rate limiting conversations.view. Retry in 12s."),
        CapError::RateLimited {
            retry_after_ms: 12_000
        }
    );
    assert_eq!(
        case("Could not reach Slack for conversations.view: fetch failed"),
        CapError::Offline
    );
    assert!(matches!(
        case("No channel matches \"x\". If it is new, run: slackcli refresh"),
        CapError::NotFound { .. }
    ));
    assert!(matches!(
        case("Slack refused conversations.replies: thread_not_found"),
        CapError::NotFound { .. }
    ));
    match case("Slack refused conversations.view: restricted_action") {
        CapError::Provider { code, message } => {
            assert_eq!(code, "restricted_action");
            assert!(message.contains("conversations.view"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_program_and_odd_output_are_provider_errors() {
    struct Gone;
    impl Runner for Gone {
        fn run(&self, _: &[String]) -> Result<Output, RunError> {
            Err(RunError::NotInstalled("slackcli".into()))
        }
    }
    let p = SlackMessaging::new(Gone, config());
    assert!(
        matches!(p.whoami(), Err(CapError::Provider { code, .. }) if code == "slackcli_missing")
    );
    let odd = Fixtures::default().with("whoami", Output::ok("not json"));
    assert!(
        matches!(provider(&odd).whoami(), Err(CapError::Provider { code, .. }) if code == "bad_output")
    );
    let real = CommandRunner::new("slackcli-that-does-not-exist-7f3a");
    assert_eq!(
        real.run(&["whoami".into()]),
        Err(RunError::NotInstalled(
            "slackcli-that-does-not-exist-7f3a".into()
        ))
    );
}

#[test]
fn a_subscription_tells_a_new_message_once_and_stops_when_dropped() {
    let slack = Arc::new(FakeSlack::new());
    struct Shared(Arc<FakeSlack>);
    impl Runner for Shared {
        fn run(&self, args: &[String]) -> Result<Output, RunError> {
            self.0.run(args)
        }
    }
    let mut config = SlackConfig::new(
        "acme",
        vec![ChannelSpec::new("C01", "general", ChannelKind::Public)],
    );
    config.poll_every = Duration::from_millis(20);
    let p = SlackMessaging::new(Shared(slack.clone()), config);
    slack.post_as("C01", "sam", "old news");
    let sub = p.subscribe(&Filter::default()).unwrap();
    slack.post_as("C01", "sam", "fresh");
    let event = sub.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(event.kind, EventKind::Posted);
    assert_eq!(event.data.unwrap().text, "fresh");
    assert!(
        sub.recv_timeout(Duration::from_millis(150)).is_err(),
        "old news and repeats do not arrive"
    );
    drop(sub);
    std::thread::sleep(Duration::from_millis(100));
    let before = slack.calls().len();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(slack.calls().len(), before, "the polling stopped");
}

#[test]
fn a_link_gives_its_channel_message_and_thread() {
    let link = parse_permalink(
        "https://app.slack.com/client/T01/C01ABC/p1760000110000400?thread_ts=1760000080.000200",
    )
    .unwrap();
    assert_eq!(
        (
            link.channel.as_str(),
            link.ts.as_str(),
            link.thread_ts.as_deref()
        ),
        ("C01ABC", "1760000110.000400", Some("1760000080.000200"))
    );
    assert!(parse_permalink("https://example.com/nothing").is_none());
    assert!(parse_permalink("https://app.slack.com/client/T01/C01ABC/p17600001100004").is_none());
}

#[test]
fn text_becomes_slack_markup_and_cannot_ping() {
    let cases = [
        ("hello", "hello"),
        (
            "**bold** and *italic* and ~~gone~~",
            "*bold* and _italic_ and ~gone~",
        ),
        ("`a < b` and ```x & y```", "`a &lt; b` and ```x &amp; y```"),
        (
            "[docs](https://example.com/a?b=1)",
            "<https://example.com/a?b=1|docs>",
        ),
        ("[@Sam](messaging:slack:acme:user/U02) look", "<@U02> look"),
        ("[#general](messaging:slack:acme:C01)", "<#C01>"),
        ("[x](javascript:alert(1))", "[x](javascript:alert(1))"),
        ("<!channel> <@U02> &", "&lt;!channel&gt; &lt;@U02&gt; &amp;"),
        ("> quoted\n> more\na > b", "> quoted\n> more\na &gt; b"),
        ("* item *not italic *", "* item *not italic *"),
        ("2 * 3 * 4", "2 * 3 * 4"),
        ("**unclosed", "**unclosed"),
    ];
    for (input, expected) in cases {
        assert_eq!(to_mrkdwn(input), expected, "{input}");
    }
}

/// Reads from a real workspace through the real `slackcli`. It sends nothing. Run it by hand on the machine that holds
/// the login: `ATELIER_SLACKCLI=slackcli cargo test -p atelier-slack -- --ignored live`.
#[test]
#[ignore = "needs slackcli and a Slack login"]
fn live_whoami_and_workspace_are_readable() {
    let program = std::env::var("ATELIER_SLACKCLI").unwrap_or_else(|_| "slackcli".into());
    let p = SlackMessaging::new(
        CommandRunner::new(program),
        SlackConfig::new("live", Vec::new()),
    );
    let me = p.whoami().expect("whoami");
    assert!(!me.id.is_empty());
    assert!(!p.workspace().expect("workspace").name.is_empty());
}
