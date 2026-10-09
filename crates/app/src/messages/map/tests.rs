use atelier_capabilities::{
    Actor,
    messaging::{
        Attachment, AttachmentKind, Channel, ChannelKind, Formatting, MemoryMessaging, Message,
        MessagingProvider, NewMessage, Reaction,
    },
};

use super::*;

fn provider() -> (MemoryMessaging, atelier_capabilities::Ref) {
    let p = MemoryMessaging::new("acme").with_me("Alex");
    let channel = p.add_channel("general", ChannelKind::Public);
    (p, channel)
}

fn said(p: &MemoryMessaging, channel: &atelier_capabilities::Ref, text: &str) -> Message {
    p.send(&NewMessage::to(channel, text), &p.whoami().unwrap())
        .unwrap()
}

#[test]
fn a_message_becomes_a_line_with_its_author_letter_and_age() {
    let (p, channel) = provider();
    let mut message = said(&p, &channel, "hello **team**");
    message.created_at = 1_000_000 * 1000;
    let line = line_of(&message, Formatting::Rich, 1_000_000 + 3 * 3600);
    assert_eq!(line.author, "Alex");
    assert_eq!(line.letter, "A");
    assert_eq!(line.time, "3h");
    assert_eq!(line.origin, None, "a person's own message has no origin");
    assert!(!line.agent);
    assert_eq!(line.body, Body::Markdown("hello **team**".into()));
    assert_eq!(line.reference, message.reference);
}

#[test]
fn an_agents_message_names_whose_agent_sent_it() {
    let (p, channel) = provider();
    let agent = Actor::agent("claude", "Claude", "me");
    let message = p
        .send(&NewMessage::to(&channel, "tests pass"), &agent)
        .unwrap();
    let line = line_of(&message, Formatting::Rich, 0);
    assert!(line.agent);
    assert_eq!(line.author, "Claude");
    assert_eq!(line.origin.as_deref(), Some("Alex's agent"));
}

#[test]
fn the_letter_of_a_name_is_its_first_letter_or_digit() {
    assert_eq!(letter_of("ana"), "A");
    assert_eq!(letter_of("  ébène"), "É");
    assert_eq!(letter_of("#general"), "G");
    assert_eq!(letter_of("42nd"), "4");
    assert_eq!(letter_of("***"), "?");
    assert_eq!(letter_of(""), "?");
}

#[test]
fn reactions_and_files_show_what_the_provider_gave() {
    let (p, channel) = provider();
    let mut message = said(&p, &channel, "report");
    message.reactions = vec![Reaction {
        name: "thumbsup".into(),
        count: 3,
        by: Vec::new(),
        me: true,
    }];
    message.attachments = vec![
        Attachment {
            id: "1".into(),
            name: "plan.pdf".into(),
            mime: None,
            size: Some(2048),
            url: "https://x.test/plan.pdf".into(),
            kind: AttachmentKind::File,
        },
        Attachment {
            id: "2".into(),
            name: "shot.png".into(),
            mime: None,
            size: None,
            url: "https://x.test/shot.png".into(),
            kind: AttachmentKind::Image,
        },
    ];
    message.reply_count = 2;
    let line = line_of(&message, Formatting::Rich, 0);
    assert_eq!(
        line.reactions,
        vec![Chip {
            name: "thumbsup".into(),
            count: 3,
            mine: true
        }]
    );
    assert_eq!(line.files[0].name, "plan.pdf");
    assert_eq!(line.files[0].detail, "2 KB");
    assert_eq!(
        line.files[1].detail, "image",
        "no size: the kind says what it is"
    );
    assert_eq!(line.replies, 2);
}

#[test]
fn sizes_read_in_one_unit() {
    assert_eq!(size_words(0), "0 B");
    assert_eq!(size_words(999), "999 B");
    assert_eq!(size_words(2048), "2 KB");
    assert_eq!(size_words(1_572_864), "1.5 MB");
    assert_eq!(size_words(5 * 1024 * 1024 * 1024), "5 GB");
}

#[test]
fn text_is_never_run_as_markup() {
    // A tag stays text.
    assert_eq!(
        safe_markdown("a <script>alert(1)</script> b"),
        r"a \<script>alert(1)\</script> b"
    );
    // A link that is not on the web is its words: a script, a file, an app, a mention of a person.
    assert_eq!(safe_markdown("[run](javascript:alert(1))"), "run");
    assert_eq!(safe_markdown("[open](file:///etc/passwd)"), "open");
    assert_eq!(
        safe_markdown("hi [@Sam](messaging:slack:acme:user/U02)!"),
        "hi @Sam!"
    );
    // A web link stays a link.
    assert_eq!(
        safe_markdown("[docs](https://example.com/a?b=c)"),
        "[docs](https://example.com/a?b=c)"
    );
    // An image is not fetched: its words stand in.
    assert_eq!(
        safe_markdown("![a cat](https://example.com/cat.png)"),
        "a cat"
    );
    assert_eq!(safe_markdown("![](https://example.com/cat.png)"), "image");
    // A heading mark is a plain character: the subset has no headings.
    assert_eq!(safe_markdown("# loud"), r"\# loud");
    // Code is code: nothing inside it is touched.
    assert_eq!(
        safe_markdown("use `[x](javascript:1)` and `<b>`"),
        "use `[x](javascript:1)` and `<b>`"
    );
    assert_eq!(
        safe_markdown("```\n<b>[x](file:///a)</b>\n```\nafter <i>"),
        "```\n<b>[x](file:///a)</b>\n```\nafter \\<i>"
    );
    // The subset the spec names passes through.
    let kept = "**bold** *it* ~~no~~ `code`\n> quote\n- item";
    assert_eq!(safe_markdown(kept), kept);
}

#[test]
fn a_provider_without_formatting_shows_the_text_as_typed() {
    let (p, channel) = provider();
    let message = said(&p, &channel, "*not bold* <b>x</b> [a](b)");
    let line = line_of(&message, Formatting::Plain, 0);
    assert_eq!(line.body, Body::Plain("*not bold* <b>x</b> [a](b)".into()));
}

#[test]
fn a_channel_row_says_its_kind_and_whether_it_has_unread() {
    let channel = |kind, unread| Channel {
        reference: "messaging:memory:acme:C1".parse().unwrap(),
        name: "general".into(),
        kind,
        topic: None,
        archived: false,
        member_count: None,
        unread,
        created_at: None,
        raw: None,
    };
    assert!(row_of(&channel(ChannelKind::Public, Some(3))).unread);
    assert!(!row_of(&channel(ChannelKind::Public, Some(0))).unread);
    assert!(
        !row_of(&channel(ChannelKind::Public, None)).unread,
        "a provider that does not say has nothing unread"
    );
    assert_eq!(
        row_of(&channel(ChannelKind::Private, None)).group,
        Group::Channels
    );
    assert_eq!(row_of(&channel(ChannelKind::Dm, None)).group, Group::Direct);
    assert_eq!(
        row_of(&channel(ChannelKind::GroupDm, None)).group,
        Group::Direct
    );
}
