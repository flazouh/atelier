use atelier_capabilities::messaging::{
    AttachmentKind, ChannelKind, ChannelQuery, Filter, MessagingProvider, SearchQuery,
};
use atelier_capabilities::{CapError, Ref};

use super::support::{Fixtures, GENERAL, GUILD, M1, M2, M3, M5, call, general, message, provider};
use crate::{DiscordConfig, DiscordMessaging, snowflake_ms};

#[test]
fn channels_keep_the_ones_with_messages_and_map_their_kinds() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures)
        .channels(&ChannelQuery::default())
        .unwrap();
    let seen: Vec<(&str, ChannelKind)> = page
        .items
        .iter()
        .map(|c| (c.name.as_str(), c.kind))
        .collect();
    assert_eq!(
        seen,
        [
            ("general", ChannelKind::Public),
            ("announcements", ChannelKind::Public),
            ("release-notes", ChannelKind::Public),
            ("staff-only", ChannelKind::Private),
        ],
        "a category and a voice channel are left out"
    );
    assert_eq!(page.items[0].reference, general());
    assert_eq!(
        call(&fixtures, 0),
        "channels --json -n 100 --offset 0 1100000000000000001"
    );
    assert_eq!(
        page.items[0].created_at,
        snowflake_ms(GENERAL),
        "the time is in the id"
    );
    assert!(page.items[0].raw.is_some(), "the row is kept");
}

#[test]
fn dms_are_in_the_account_dm_and_filter_by_kind_and_text() {
    let fixtures = Fixtures::the_usual();
    let mut config = DiscordConfig::new(GUILD);
    config.include_dms = true;
    let p = DiscordMessaging::new(fixtures.clone(), config);
    let dms = p
        .channels(&ChannelQuery {
            kinds: vec![ChannelKind::Dm, ChannelKind::GroupDm],
            ..ChannelQuery::default()
        })
        .unwrap();
    let seen: Vec<(String, ChannelKind)> = dms
        .items
        .iter()
        .map(|c| (c.reference.to_string(), c.kind))
        .collect();
    assert_eq!(
        seen,
        [
            (
                "messaging:discord:dm:1300000000000000001".to_string(),
                ChannelKind::Dm
            ),
            (
                "messaging:discord:dm:1300000000000000002".to_string(),
                ChannelKind::GroupDm
            ),
        ]
    );
    let by_text = p
        .channels(&ChannelQuery {
            text: Some("KIM".into()),
            ..ChannelQuery::default()
        })
        .unwrap();
    assert_eq!(by_text.items.len(), 1);
}

#[test]
fn channels_page_by_offset() {
    let p = provider(&Fixtures::the_usual());
    let first = p
        .channels(&ChannelQuery {
            limit: Some(3),
            ..ChannelQuery::default()
        })
        .unwrap();
    assert_eq!(first.items.len(), 3);
    assert_eq!(first.next_cursor.as_deref(), Some("3"));
    let rest = p
        .channels(&ChannelQuery {
            limit: Some(3),
            cursor: first.next_cursor,
            ..ChannelQuery::default()
        })
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert!(rest.next_cursor.is_none());
}

#[test]
fn history_hides_replies_newest_first_and_counts_them_on_the_root() {
    let fixtures = Fixtures::the_usual();
    let page = provider(&fixtures).history(&general(), None, None).unwrap();
    let texts: Vec<&str> = page.items.iter().map(|m| m.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "",
            "Who can review the release notes?",
            "Morning. The build is green."
        ],
        "the two replies are not here"
    );
    let root = &page.items[1];
    assert_eq!(root.reply_count, 2, "a reply and a reply to it");
    assert_eq!(root.reference, message(M2));
    assert_eq!(root.channel, general());
    assert_eq!(root.created_at, 1_760_000_060_000);
    assert_eq!(root.author.id, "1400000000000000001");
    assert_eq!(page.items[2].author.name, "Sam");
    assert!(page.next_cursor.is_none());
    assert_eq!(call(&fixtures, 0), format!("read --json -n 50 {GENERAL}"));
}

#[test]
fn history_maps_attachments_and_keeps_the_row() {
    let page = provider(&Fixtures::the_usual())
        .history(&general(), None, None)
        .unwrap();
    let files = &page.items[0].attachments;
    assert_eq!(files.len(), 2);
    assert_eq!(
        (files[0].kind, files[0].name.as_str(), files[0].size),
        (AttachmentKind::Image, "shot.png", Some(421_888))
    );
    assert_eq!(files[1].kind, AttachmentKind::File);
    assert_eq!(files[0].url, "https://cdn.example.test/shot.png");
    assert!(page.items[0].raw.is_some());
}

#[test]
fn history_turns_mentions_and_emoji_into_the_subset() {
    let p = provider(&Fixtures::the_usual());
    let thread = p.thread(&message(M2), None).unwrap();
    assert_eq!(
        thread.items[1].text,
        format!(
            "I can. [@1400000000000000001](messaging:discord:{GUILD}:user/1400000000000000001) see \
             [#1200000000000000004](messaging:discord:{GUILD}:1200000000000000004) and :party:"
        )
    );
}

#[test]
fn history_goes_back_with_the_id_of_its_oldest_row_when_the_page_is_full() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let page = p.history(&general(), None, Some(5)).unwrap();
    // The fixture holds five rows, so a page of five may have more behind it.
    assert_eq!(
        page.next_cursor.as_deref(),
        None,
        "the tool said there is no more"
    );
    let full = Fixtures::the_usual().with(
        "read",
        crate::Output::ok(
            super::support::READ.replace(r#""hasMore": false,"#, r#""hasMore": true,"#),
        ),
    );
    let p = provider(&full);
    let page = p.history(&general(), None, Some(5)).unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some(M1));
    p.history(&general(), page.next_cursor.as_deref(), Some(5))
        .unwrap();
    assert_eq!(
        call(&full, 1),
        format!("read --json -n 5 --before {M1} {GENERAL}")
    );
    assert!(matches!(
        p.history(&general(), Some("later"), None),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn a_thread_is_the_root_and_the_replies_that_lead_to_it_oldest_first() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let page = p.thread(&message(M2), None).unwrap();
    let ids: Vec<String> = page.items.iter().map(|m| m.reference.id.clone()).collect();
    assert_eq!(
        ids,
        [M2, M3, M5].map(|m| format!("{GENERAL}:{m}")),
        "the root, a reply, and a reply to the reply"
    );
    assert_eq!(page.items[0].reply_count, 2);
    assert_eq!(page.items[0].parent, None);
    assert_eq!(page.items[2].parent, Some(message(M2)), "under the root");
    assert!(page.next_cursor.is_none());
    assert_eq!(
        call(&fixtures, 0),
        format!("read --json -n 1 https://discord.com/channels/{GUILD}/{GENERAL}/{M2}")
    );
    assert_eq!(
        call(&fixtures, 1),
        format!("read --json -n 100 --after {M2} {GENERAL}")
    );
}

#[test]
fn the_thread_of_a_reply_is_the_thread_of_its_root() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let from_reply = p.thread(&message(M5), None).unwrap();
    let from_root = p.thread(&message(M2), None).unwrap();
    assert_eq!(from_reply.items, from_root.items);
}

#[test]
fn a_message_that_is_not_there_is_not_found() {
    let p = provider(&Fixtures::the_usual());
    let missing = "1425769339289600099";
    assert!(matches!(
        p.thread(&message(missing), None),
        Err(CapError::NotFound { .. })
    ));
}

#[test]
fn search_runs_in_the_server_and_pages_by_offset() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let page = p
        .search(&SearchQuery {
            text: "build".into(),
            channel: Some(general()),
            from: Some("1500000000000000001".into()),
            limit: Some(10),
            cursor: Some("4".into()),
        })
        .unwrap();
    assert_eq!(
        call(&fixtures, 0),
        format!(
            "search --json -n 10 --offset 4 --server {GUILD} --channel {GENERAL} --from 1500000000000000001 -- build"
        )
    );
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].text, "Morning. The build is green.");
    assert_eq!(
        page.next_cursor.as_deref(),
        Some("1"),
        "the tool's next offset"
    );
}

#[test]
fn search_text_that_looks_like_an_option_stays_text() {
    let fixtures = Fixtures::the_usual();
    provider(&fixtures)
        .search(&SearchQuery {
            text: "--yes".into(),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(call(&fixtures, 0).ends_with(" -- --yes"));
}

#[test]
fn the_workspace_is_the_server_and_whoami_is_the_login() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let workspace = p.workspace().unwrap();
    assert_eq!(workspace.name, "Acme");
    assert_eq!(
        workspace.reference.to_string(),
        format!("messaging:discord:{GUILD}:workspace")
    );
    let me = p.whoami().unwrap();
    assert_eq!(
        (me.id.as_str(), me.name.as_str()),
        ("1400000000000000001", "Alex")
    );
    p.whoami().unwrap();
    let whoami_runs = fixtures.calls().iter().filter(|c| c[0] == "whoami").count();
    assert_eq!(whoami_runs, 1, "the login is asked once");
}

#[test]
fn the_dm_account_has_a_name_without_a_command() {
    let fixtures = Fixtures::the_usual();
    let p = DiscordMessaging::new(fixtures.clone(), DiscordConfig::new("dm"));
    assert_eq!(p.workspace().unwrap().name, "Direct messages");
    assert!(fixtures.calls().is_empty());
}

#[test]
fn a_reference_of_another_provider_or_account_is_invalid() {
    let p = provider(&Fixtures::the_usual());
    for text in [
        format!("messaging:slack:{GUILD}:{GENERAL}"),
        format!("messaging:discord:1100000000000000099:{GENERAL}"),
        format!("messaging:discord:dm:{GENERAL}"),
    ] {
        let other: Ref = text.parse().unwrap();
        assert!(
            matches!(p.history(&other, None, None), Err(CapError::Invalid { .. })),
            "{text}"
        );
    }
}

#[test]
fn an_id_that_cannot_be_an_id_is_not_found_and_runs_no_command() {
    let fixtures = Fixtures::the_usual();
    let p = provider(&fixtures);
    let nowhere: Ref = format!("messaging:discord:{GUILD}:NOPE").parse().unwrap();
    assert!(matches!(
        p.history(&nowhere, None, None),
        Err(CapError::NotFound { .. })
    ));
    assert!(matches!(
        p.subscribe(&Filter {
            channels: vec![nowhere]
        }),
        Err(CapError::NotFound { .. })
    ));
    assert!(fixtures.calls().is_empty());
}

#[test]
fn a_subscription_for_more_channels_than_the_cap_is_refused() {
    let fixtures = Fixtures::the_usual();
    let mut config = DiscordConfig::new(GUILD);
    config.max_polled = 2;
    let p = DiscordMessaging::new(fixtures.clone(), config);
    assert!(matches!(
        p.subscribe(&Filter::default()),
        Err(CapError::Invalid { .. })
    ));
    let before = fixtures.calls().len();
    assert!(before <= 1, "only the channel list ran, no polling started");
}
