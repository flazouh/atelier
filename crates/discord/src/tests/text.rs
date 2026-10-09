use crate::{from_discord, to_discord};

const ACME: &str = "1100000000000000001";

#[test]
fn the_subset_goes_through_as_it_is() {
    let text = "**bold** *it* ~~no~~ `code`\n> quote\n- one\n- two";
    assert_eq!(to_discord(text), text);
}

#[test]
fn a_link_of_a_person_a_channel_and_a_message_becomes_discord_markup() {
    assert_eq!(
        to_discord(&format!(
            "[@Sam](messaging:discord:{ACME}:user/1500000000000000001) look"
        )),
        "<@1500000000000000001> look"
    );
    assert_eq!(
        to_discord(&format!(
            "[#dev](messaging:discord:{ACME}:1200000000000000001)"
        )),
        "<#1200000000000000001>"
    );
    assert_eq!(
        to_discord(&format!(
            "[this](messaging:discord:{ACME}:1200000000000000001:1425768332656640001)"
        )),
        format!(
            "this (https://discord.com/channels/{ACME}/1200000000000000001/1425768332656640001)"
        )
    );
    assert_eq!(
        to_discord("[a dm](messaging:discord:dm:1300000000000000001:1425768332656640001)"),
        "a dm (https://discord.com/channels/@me/1300000000000000001/1425768332656640001)"
    );
}

#[test]
fn a_web_link_stays_beside_its_label_because_discord_shows_no_link_text() {
    assert_eq!(
        to_discord("[the docs](https://example.test/a)"),
        "the docs (https://example.test/a)"
    );
    assert_eq!(
        to_discord("[https://example.test](https://example.test)"),
        "https://example.test"
    );
}

#[test]
fn a_link_that_means_nothing_stays_as_typed() {
    for text in [
        "[x](javascript:alert(1))",
        "[x](messaging:slack:acme:C01)",
        "[x](messaging:discord:1100000000000000001:user/not-a-number)",
        "[x] (y)",
    ] {
        assert_eq!(to_discord(text), text);
    }
}

#[test]
fn no_text_can_ping_everyone_here_or_a_role() {
    let out = to_discord(
        "@everyone and @here, @Everyone, @HERE, <@&123456789012345678>, <@111111111111111111>, <@!111111111111111111>",
    );
    for ping in [
        "@everyone",
        "@here",
        "@Everyone",
        "@HERE",
        "<@&",
        "<@1",
        "<@!",
    ] {
        assert!(!out.contains(ping), "{ping} in {out:?}");
    }
    assert_eq!(
        out.replace('\u{200b}', ""),
        "@everyone and @here, @Everyone, @HERE, <@&123456789012345678>, <@111111111111111111>, <@!111111111111111111>"
    );
}

#[test]
fn a_mention_that_a_link_made_is_not_broken_by_the_check() {
    assert_eq!(
        to_discord(&format!(
            "[@Sam](messaging:discord:{ACME}:user/1500000000000000001) @everyone"
        )),
        "<@1500000000000000001> @\u{200b}everyone"
    );
}

#[test]
fn code_is_copied_as_written_but_an_unclosed_fence_is_not_code() {
    assert_eq!(to_discord("`@everyone`"), "`@everyone`");
    assert_eq!(to_discord("```\n@everyone\n```"), "```\n@everyone\n```");
    let open = to_discord("```\n@everyone");
    assert!(!open.contains("@everyone"), "{open:?}");
}

#[test]
fn text_from_discord_gets_references_for_mentions_and_names_for_emoji() {
    assert_eq!(
        from_discord(
            "hi <@1500000000000000001> and <@!1500000000000000002> in <#1200000000000000001>",
            ACME
        ),
        format!(
            "hi [@1500000000000000001](messaging:discord:{ACME}:user/1500000000000000001) and \
             [@1500000000000000002](messaging:discord:{ACME}:user/1500000000000000002) in \
             [#1200000000000000001](messaging:discord:{ACME}:1200000000000000001)"
        )
    );
    assert_eq!(
        from_discord(
            "<@&1600000000000000001> <:party:1600000000000000002> <a:spin:1600000000000000003>",
            ACME
        ),
        "@role :party: :spin:"
    );
    assert_eq!(
        from_discord("see <https://example.test/a>", ACME),
        "see https://example.test/a"
    );
}

#[test]
fn text_from_discord_that_is_not_markup_is_left_alone() {
    for text in [
        "a < b > c",
        "<b>bold</b>",
        "<@abc>",
        "5 <3 you",
        "<",
        "x <@",
    ] {
        assert_eq!(from_discord(text, ACME), text);
    }
}
