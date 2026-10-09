//! The Chat and Mail groups of the Accounts section, driven as a reader drives them: type, Test, Save, Forget.
use std::{os::unix::fs::PermissionsExt, sync::Arc};

use atelier_capabilities::{
    CapError, CapResult, Ref,
    mail::{MailProvider, MemoryMail},
    messaging::{MemoryMessaging, MessagingProvider, NewMessage},
};
use atelier_settings::{DiscordSaved, GmailSaved, Settings, SlackSaved, secrets::InMemory};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::{Checked, open, open_with, press, settle};
use crate::{accounts::Row, settings_pane::SettingsPane};

pub(super) fn slack(saved: &SlackSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    match saved.workspace.as_str() {
        "acme" => Ok(Arc::new(MemoryMessaging::new("acme"))),
        "loggedout" => Err(CapError::NotSignedIn),
        "missing" => Err(CapError::Provider {
            code: "slackcli_missing".into(),
            message: "`slackcli` is not installed".into(),
        }),
        _ => Err(CapError::invalid("workspace")),
    }
}

pub(super) fn discord(saved: &DiscordSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    match saved.server.as_str() {
        "Acme" => Ok(Arc::new(
            MemoryMessaging::new("1100000000000000001").with_me("Alex"),
        )),
        _ => Err(CapError::NotSignedIn),
    }
}

pub(super) fn gmail(saved: &GmailSaved) -> CapResult<Arc<dyn MailProvider>> {
    match saved.address.as_str() {
        "me@acme.test" => Ok(Arc::new(MemoryMail::new("me@acme.test"))),
        _ => Err(CapError::NotSignedIn),
    }
}

fn type_into(pane: &Entity<SettingsPane>, name: &str, text: &str, cx: &mut VisualTestContext) {
    let field = pane.read_with(cx, |p, _| {
        if name.starts_with("gmail") {
            p.mail_field(name)
        } else {
            p.chat_field(name)
        }
    });
    field.update_in(cx, |field, window, cx| field.set_value(text, window, cx));
}

fn slack_check(pane: &Entity<SettingsPane>, cx: &mut VisualTestContext) -> Checked {
    pane.read_with(cx, |p, _| p.accounts.chat.slack_check.clone())
}

/// The page, in a window tall enough to show every card: the Chat and Mail cards lie below the first fold.
fn tall(
    opened: (
        Entity<SettingsPane>,
        crate::capability_hub::CapabilityHub,
        &mut VisualTestContext,
    ),
) -> (
    Entity<SettingsPane>,
    crate::capability_hub::CapabilityHub,
    &mut VisualTestContext,
) {
    let (pane, hub, cx) = opened;
    cx.simulate_resize(gpui_kit::size(
        atelier_ui::scale::px(900.),
        atelier_ui::scale::px(3600.),
    ));
    settle(cx);
    (pane, hub, cx)
}

fn found(name: &str, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| crate::control::find(name, cx)).is_some()
}

#[gpui_kit::test]
fn the_section_has_a_chat_group_with_slack_and_discord_and_a_mail_group_with_gmail(
    cx: &mut TestAppContext,
) {
    let (_pane, _hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    for name in [
        "slack-test",
        "slack-save",
        "discord-test",
        "discord-save",
        "gmail-test",
        "gmail-save",
    ] {
        assert!(found(name, cx), "{name} is drawn");
    }
    for name in ["slack-forget", "discord-forget", "gmail-forget"] {
        assert!(!found(name, cx), "{name}: nothing to forget yet");
    }
    assert!(
        cx.debug_bounds("discord-dms").is_some() && cx.debug_bounds("discord-writes").is_some()
    );
}

#[gpui_kit::test]
fn a_slack_workspace_is_saved_with_its_channels_and_its_provider_built_and_forgetting_leaves_it_out(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    type_into(&pane, "slack-workspace", " acme ", cx);
    type_into(&pane, "slack-channels", "general, #random ,C01", cx);
    press("slack-save", cx);

    let saved = pane
        .read_with(cx, |p, _| p.accounts.saved.slack.clone())
        .expect("kept");
    assert_eq!(saved.workspace, "acme");
    assert_eq!(saved.channels, ["general", "#random", "C01"]);
    assert_eq!(
        saved.person.as_deref(),
        Some("Me"),
        "the name the tool gave"
    );
    assert_eq!(hub.rows().slack, Row::Connected("Me".into()));
    let accounts: Vec<_> = hub
        .messaging_providers()
        .iter()
        .map(|p| p.account().to_string())
        .collect();
    assert_eq!(
        accounts,
        ["acme"],
        "the Messages screen and the gateway read it from the hub"
    );

    pane.update(cx, |p, cx| p.show(super::Section::Accounts, cx));
    settle(cx);
    press("slack-forget", cx);
    assert!(pane.read_with(cx, |p, _| p.accounts.saved.slack.is_none()));
    assert!(hub.messaging_providers().is_empty());
    assert_eq!(hub.rows().slack, Row::Off);
}

#[gpui_kit::test]
fn a_test_names_the_person_and_keeps_and_builds_nothing(cx: &mut TestAppContext) {
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    type_into(&pane, "slack-workspace", "acme", cx);
    press("slack-test", cx);
    assert_eq!(slack_check(&pane, cx), Checked::Works("Me".into()));
    assert!(pane.read_with(cx, |p, _| p.accounts.saved.slack.is_none()));
    assert!(hub.messaging_providers().is_empty());
}

#[gpui_kit::test]
fn a_logged_out_or_missing_tool_is_said_in_plain_words_and_the_row_shows_it_after_a_save(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    type_into(&pane, "slack-workspace", "loggedout", cx);
    press("slack-test", cx);
    let Checked::Refused(words) = slack_check(&pane, cx) else {
        panic!("logged out was accepted")
    };
    assert!(words.contains("slackcli login"), "{words}");

    type_into(&pane, "slack-workspace", "missing", cx);
    press("slack-save", cx);
    let Checked::Refused(words) = slack_check(&pane, cx) else {
        panic!("a missing tool was accepted")
    };
    assert!(
        words.contains("slackcli is not installed")
            && words.contains("github.com/flazouh/slackcli"),
        "{words}"
    );
    assert!(
        hub.messaging_providers().is_empty(),
        "no provider for a tool that is not there"
    );
    assert!(matches!(hub.rows().slack, Row::Failed(_)));
    assert!(
        pane.read_with(cx, |p, _| p.accounts.saved.slack.is_some()),
        "kept: it works once the tool is installed"
    );
}

#[gpui_kit::test]
fn allow_sending_is_off_until_the_reader_turns_it_on_and_the_risk_is_said_beside_it(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    assert!(
        !pane.read_with(cx, |p, _| p.accounts.chat.discord_writes),
        "off in a new block"
    );
    type_into(&pane, "discord-server", "Acme", cx);
    press("discord-save", cx);
    let saved = pane
        .read_with(cx, |p, _| p.accounts.saved.discord.clone())
        .expect("kept");
    assert!(!saved.allow_writes && !saved.include_dms, "{saved:?}");
    assert_eq!(hub.rows().discord, Row::Connected("Alex".into()));

    pane.update(cx, |p, cx| p.show(super::Section::Accounts, cx));
    settle(cx);
    let at = cx
        .debug_bounds("discord-writes")
        .expect("the switch is drawn")
        .center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(cx);
    assert!(
        pane.read_with(cx, |p, _| p.accounts.chat.discord_writes),
        "the switch turned on"
    );
    press("discord-save", cx);
    assert!(pane.read_with(cx, |p, _| {
        p.accounts
            .saved
            .discord
            .as_ref()
            .is_some_and(|d| d.allow_writes)
    }));
}

#[gpui_kit::test]
fn a_discord_provider_built_from_a_default_block_cannot_send(cx: &mut TestAppContext) {
    // The real way in, over a tool that answers like discordcli: a name is looked up, then a send is refused.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discordcli");
    std::fs::write(
        &path,
        r#"#!/bin/sh
case "$1" in
servers) echo '{"rows":[{"id":"1100000000000000001","name":"Acme"}],"hasMore":false}' ;;
whoami) echo '{"id":"1400000000000000001","username":"alex_u","name":"Alex"}' ;;
*) echo "unexpected: $1" >&2; exit 1 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    cx.update(|_, cx| {
        cx.set_global(crate::accounts::AccountServices {
            secrets: Arc::new(InMemory::default()),
            ..crate::accounts::AccountServices::system()
        })
    });
    type_into(&pane, "discord-server", "Acme", cx);
    type_into(&pane, "discord-program", &path.to_string_lossy(), cx);
    press("discord-save", cx);

    assert_eq!(hub.rows().discord, Row::Connected("Alex".into()));
    let provider = hub.messaging_providers().into_iter().next().expect("built");
    assert_eq!(provider.account(), "1100000000000000001");
    let channel = Ref {
        capability: "messaging".into(),
        provider: "discord".into(),
        account: "1100000000000000001".into(),
        id: "1200000000000000001".into(),
    };
    let sent = provider.send(
        &NewMessage::to(&channel, "hi"),
        &atelier_capabilities::Actor::person("me", "Me"),
    );
    assert!(
        matches!(&sent, Err(CapError::Provider { code, .. }) if code == "read_only"),
        "{sent:?}"
    );
}

#[gpui_kit::test]
fn gmail_is_saved_with_its_address_and_where_it_runs_and_registers_a_mail_provider(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    assert!(
        cx.debug_bounds("gmail-host").is_none(),
        "no host field while it runs here"
    );
    type_into(&pane, "gmail-address", "me@acme.test", cx);
    press("gmail-save", cx);

    let saved = pane
        .read_with(cx, |p, _| p.accounts.saved.gmail.clone())
        .expect("kept");
    assert_eq!(
        (saved.address.as_str(), saved.host.as_str()),
        ("me@acme.test", "")
    );
    assert_eq!(hub.rows().gmail, Row::Connected("me@acme.test".into()));
    let accounts: Vec<_> = hub
        .mail_providers()
        .iter()
        .map(|p| p.account().to_string())
        .collect();
    assert_eq!(accounts, ["me@acme.test"]);

    press("gmail-forget", cx);
    assert!(hub.mail_providers().is_empty());
    assert!(pane.read_with(cx, |p, _| p.accounts.saved.gmail.is_none()));
}

#[gpui_kit::test]
fn over_ssh_asks_for_a_host_and_keeps_it(cx: &mut TestAppContext) {
    let (pane, _hub, cx) = tall(open(&Arc::new(InMemory::default()), cx));
    let at = cx
        .debug_bounds("gmail-ssh")
        .expect("the choice is drawn")
        .center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(cx);
    assert!(
        cx.debug_bounds("gmail-host").is_some()
            || pane.read_with(cx, |p, _| p.accounts.mail.over_ssh)
    );
    type_into(&pane, "gmail-address", "me@acme.test", cx);
    type_into(&pane, "gmail-host", "mac", cx);
    let draft = pane.update(cx, |p, cx| p.gmail_draft(cx)).expect("fields");
    assert_eq!(draft.host, "mac");
    let at = cx
        .debug_bounds("gmail-here")
        .expect("the choice is drawn")
        .center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(cx);
    let draft = pane.update(cx, |p, cx| p.gmail_draft(cx)).expect("fields");
    assert_eq!(draft.host, "", "a host typed and then left is not kept");
}

#[gpui_kit::test]
fn saved_blocks_fill_the_fields_when_the_page_opens(cx: &mut TestAppContext) {
    let saved = Settings {
        accounts: atelier_settings::AccountsSaved {
            slack: Some(SlackSaved {
                workspace: "acme".into(),
                channels: vec!["general".into(), "C01".into()],
                program: "/opt/slackcli".into(),
                person: None,
            }),
            discord: Some(DiscordSaved {
                server: "Acme".into(),
                include_dms: true,
                ..Default::default()
            }),
            gmail: Some(GmailSaved {
                address: "me@acme.test".into(),
                host: "mac".into(),
                program: String::new(),
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let (pane, _hub, cx) = tall(open_with(&saved, &Arc::new(InMemory::default()), cx));
    let slack = pane.update(cx, |p, cx| p.slack_draft(cx)).expect("fields");
    assert_eq!(
        (slack.workspace.as_str(), slack.program.as_str()),
        ("acme", "/opt/slackcli")
    );
    assert_eq!(slack.channels, ["general", "C01"]);
    let discord = pane
        .update(cx, |p, cx| p.discord_draft(cx))
        .expect("fields");
    assert!(discord.include_dms && !discord.allow_writes);
    let gmail = pane.update(cx, |p, cx| p.gmail_draft(cx)).expect("fields");
    assert_eq!(gmail.host, "mac");
    assert!(found("slack-forget", cx) && found("discord-forget", cx) && found("gmail-forget", cx));
}
