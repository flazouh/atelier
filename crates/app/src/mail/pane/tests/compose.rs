//! The reply box: what it needs of a provider, how a draft is saved, and that only the draft the reader sees, at the version they
//! see, is ever sent.
use atelier_capabilities::{
    CapError,
    mail::{DraftPatch, MailOperation, MailProvider, SearchQuery, mailbox_ref},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::{Fake, ME, MailPane, fake::Sent, one, open_row, press, settle, subjects};

/// Writes `text` in the reply box, as typing does.
fn write(text: &str, pane: &Entity<MailPane>, cx: &mut VisualTestContext) {
    let input = pane.read_with(cx, |p, _| p.compose_input().clone());
    pane.update_in(cx, |_, window, cx| {
        input.update(cx, |i, cx| i.set_value(text.to_string(), window, cx))
    });
    settle(pane, cx);
}

fn box_text(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> String {
    let input = pane.read_with(cx, |p, _| p.compose_input().clone());
    cx.update(|_, cx| input.read(cx).value().to_string())
}

fn status(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> String {
    pane.read_with(cx, |p, cx| {
        p.compose_facts(cx)
            .and_then(|f| f.status)
            .map(|s| s.to_string())
            .unwrap_or_default()
    })
}

fn editable(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> bool {
    pane.read_with(cx, |p, cx| p.compose_facts(cx).is_some_and(|f| f.editable))
}

#[gpui_kit::test]
fn the_reply_box_shows_who_it_answers_and_the_buttons_the_provider_lists(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    assert!(
        cx.debug_bounds("mail-composer").is_none(),
        "no thread, no box"
    );
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-composer").is_some());
    assert!(cx.debug_bounds("mail-save-draft").is_some() && cx.debug_bounds("mail-send").is_some());
    assert_eq!(
        pane.read_with(cx, |p, cx| p.compose_facts(cx).map(|f| f.to.to_string()))
            .as_deref(),
        Some("To ana@example.com"),
        "the box says who the reply goes to"
    );
}

#[gpui_kit::test]
fn the_send_button_is_there_only_when_the_provider_lists_send(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.without(MailOperation::Send);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    assert!(
        cx.debug_bounds("mail-composer").is_some(),
        "drafts are listed, so there is a box"
    );
    assert!(cx.debug_bounds("mail-save-draft").is_some());
    assert!(cx.debug_bounds("mail-send").is_none(), "no send, no Send");
}

#[gpui_kit::test]
fn a_provider_with_no_drafts_has_no_box_even_when_it_could_send(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.without(MailOperation::CreateDraft);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-composer").is_none() && cx.debug_bounds("mail-send").is_none());
}

#[gpui_kit::test]
fn an_empty_box_cannot_save_or_send(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    pane.update_in(cx, |p, window, cx| p.write_reply(true, window, cx));
    settle(&pane, cx);
    assert_eq!(
        provider.calls(MailOperation::CreateDraft) + provider.calls(MailOperation::Send),
        0
    );
    write("   ", &pane, cx);
    pane.update_in(cx, |p, window, cx| p.write_reply(false, window, cx));
    settle(&pane, cx);
    assert_eq!(
        provider.calls(MailOperation::CreateDraft),
        0,
        "blank words are not a draft"
    );
}

#[gpui_kit::test]
fn save_draft_makes_a_draft_that_answers_the_last_message_and_sends_nothing(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("Thanks, I will look.", &pane, cx);
    assert_eq!(status(&pane, cx), "Not saved");
    press("mail-save-draft", &pane, cx);
    assert_eq!(provider.calls(MailOperation::CreateDraft), 1);
    assert_eq!(
        provider.calls(MailOperation::Send),
        0,
        "a draft sends nothing"
    );
    let draft = pane
        .read_with(cx, |p, _| p.draft().cloned())
        .expect("the pane holds the draft");
    assert_eq!(draft.text, "Thanks, I will look.");
    assert_eq!(draft.subject, "Re: plan");
    assert_eq!(draft.to[0].address, "ana@example.com");
    assert_eq!(draft.created_by.id, "me", "the draft is the person's");
    assert!(draft.in_reply_to.is_some() && draft.thread.is_some());
    assert_eq!(
        provider.memory().draft(&draft.reference).unwrap(),
        draft,
        "the provider holds the same"
    );
    assert_eq!(status(&pane, cx), "Draft saved");
    // A change of the words is a change of the draft: saved with the version held, and the version moves.
    write("Thanks, I will look tonight.", &pane, cx);
    assert_eq!(status(&pane, cx), "Changes not saved");
    press("mail-save-draft", &pane, cx);
    assert_eq!(
        provider.calls(MailOperation::CreateDraft),
        1,
        "the draft is changed, not made again"
    );
    assert_eq!(provider.calls(MailOperation::UpdateDraft), 1);
    let changed = pane.read_with(cx, |p, _| p.draft().cloned()).unwrap();
    assert_eq!(changed.reference, draft.reference);
    assert_ne!(changed.version, draft.version);
    assert_eq!(changed.text, "Thanks, I will look tonight.");
    assert_eq!(status(&pane, cx), "Draft saved");
}

#[gpui_kit::test]
fn send_sends_the_draft_at_the_version_shown_and_the_box_empties(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("On my way.", &pane, cx);
    press("mail-send", &pane, cx);
    let sent = provider.sent();
    assert_eq!(sent.len(), 1, "one send");
    let Sent {
        draft,
        version,
        text,
        held_version,
    } = sent[0].clone();
    assert_eq!(
        provider.calls(MailOperation::CreateDraft),
        1,
        "the words were saved first"
    );
    // The version sent is the version of the draft that holds exactly the words in the box when Send was pressed.
    assert_eq!((text.as_str(), &version), ("On my way.", &held_version));
    assert!(
        provider.memory().draft(&draft).is_err(),
        "the draft is gone: it became a message"
    );
    assert_eq!(box_text(&pane, cx), "", "the box is empty");
    assert!(pane.read_with(cx, |p, _| p.draft().is_none()));
    // The thread has the new message, from the account, in its Sent mailbox.
    let all = provider
        .memory()
        .search(&SearchQuery {
            mailbox: Some(mailbox_ref("memory", ME, "sent")),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(all.items.len(), 1);
    assert_eq!(all.items[0].snippet, "On my way.");
    assert_eq!(
        pane.read_with(cx, |p, _| p.messages().len()),
        2,
        "the open thread was read again and has the reply"
    );
}

#[gpui_kit::test]
fn a_saved_draft_that_is_edited_is_sent_at_its_new_version_never_the_old_one(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("first words", &pane, cx);
    press("mail-save-draft", &pane, cx);
    let old = pane.read_with(cx, |p, _| p.draft().cloned()).unwrap();
    write("second words", &pane, cx);
    press("mail-send", &pane, cx);
    let sent = provider.sent();
    assert_eq!(sent.len(), 1);
    assert_ne!(
        sent[0].version, old.version,
        "the old version is not the one sent"
    );
    assert_eq!(
        provider.calls(MailOperation::UpdateDraft),
        1,
        "the words were saved before they were sent"
    );
    let message = provider
        .memory()
        .search(&SearchQuery {
            mailbox: Some(mailbox_ref("memory", ME, "sent")),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(
        message.items[0].snippet, "second words",
        "the message says what the box said"
    );
}

#[gpui_kit::test]
fn a_draft_changed_behind_the_pane_is_not_sent_and_is_shown_again(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.tamper_before_send();
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("my words", &pane, cx);
    press("mail-send", &pane, cx);
    let sent_count = provider
        .memory()
        .search(&SearchQuery {
            mailbox: Some(mailbox_ref("memory", ME, "sent")),
            ..SearchQuery::default()
        })
        .unwrap()
        .items
        .len();
    assert_eq!(
        sent_count, 0,
        "nothing left the machine: the version pressed was no longer the draft"
    );
    assert_eq!(
        provider.sent().len(),
        1,
        "one try, and no second one with the new version"
    );
    let tried = provider.sent().remove(0);
    assert_eq!(
        (tried.text.as_str(), &tried.version),
        ("my words", &tried.held_version),
        "the one try named the version the reader saw"
    );
    assert_eq!(
        box_text(&pane, cx),
        "my words (changed elsewhere)",
        "the box shows the draft as it is now"
    );
    let said = pane
        .read_with(cx, |p, _| p.said().map(str::to_string))
        .unwrap();
    assert!(said.contains("changed before it was sent"), "{said}");
}

#[gpui_kit::test]
fn a_draft_edited_elsewhere_keeps_the_readers_words_and_the_next_save_replaces_it(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("mine", &pane, cx);
    press("mail-save-draft", &pane, cx);
    let held = pane.read_with(cx, |p, _| p.draft().cloned()).unwrap();
    // An agent changes the draft.
    let agent = atelier_capabilities::Actor::agent("claude", "Claude", "me");
    provider
        .memory()
        .update_draft(
            &held.reference,
            &DraftPatch {
                text: Some("the agent's".into()),
                ..DraftPatch::default()
            },
            &held.version,
            &agent,
        )
        .unwrap();
    write("mine, longer", &pane, cx);
    press("mail-save-draft", &pane, cx);
    let said = pane
        .read_with(cx, |p, _| p.said().map(str::to_string))
        .unwrap();
    assert!(said.contains("changed elsewhere"), "{said}");
    assert_eq!(
        box_text(&pane, cx),
        "mine, longer",
        "the reader's words stay"
    );
    assert_eq!(
        provider.memory().draft(&held.reference).unwrap().text,
        "the agent's",
        "nothing was overwritten"
    );
    press("mail-save-draft", &pane, cx);
    assert_eq!(
        provider.memory().draft(&held.reference).unwrap().text,
        "mine, longer",
        "the next save, from the version now held, replaces it"
    );
}

#[gpui_kit::test]
fn a_failed_send_keeps_the_saved_draft_and_says_why(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.fail(
        MailOperation::Send,
        CapError::Provider {
            code: "E1".into(),
            message: "refused".into(),
        },
    );
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("words", &pane, cx);
    press("mail-send", &pane, cx);
    let said = pane
        .read_with(cx, |p, _| p.said().map(str::to_string))
        .unwrap();
    assert!(
        said.starts_with("Could not send") && said.contains("refused"),
        "{said}"
    );
    assert_eq!(box_text(&pane, cx), "words", "the words stay");
    assert_eq!(status(&pane, cx), "Draft saved", "and the draft is saved");
    provider.heal(MailOperation::Send);
    press("mail-send", &pane, cx);
    assert_eq!(
        provider.calls(MailOperation::CreateDraft),
        1,
        "the second try sends the draft, it does not make another"
    );
    assert_eq!(provider.sent().len(), 2);
    assert_eq!(box_text(&pane, cx), "");
}

#[gpui_kit::test]
fn a_send_the_provider_does_not_offer_removes_the_button(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.fail(MailOperation::Send, CapError::unsupported("send"));
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("words", &pane, cx);
    press("mail-send", &pane, cx);
    assert!(cx.debug_bounds("mail-send").is_none(), "Send goes");
    assert!(
        cx.debug_bounds("mail-save-draft").is_some(),
        "Save draft stays"
    );
}

#[gpui_kit::test]
fn the_words_of_a_reply_are_kept_for_each_thread_while_the_reader_looks_at_another(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["one", "two"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("for two", &pane, cx);
    open_row(1, &pane, cx);
    assert_eq!(box_text(&pane, cx), "", "another thread, another box");
    write("for one", &pane, cx);
    press("mail-save-draft", &pane, cx);
    open_row(0, &pane, cx);
    assert_eq!(
        box_text(&pane, cx),
        "for two",
        "the words are where the reader left them"
    );
    open_row(1, &pane, cx);
    assert_eq!(box_text(&pane, cx), "for one");
    assert!(
        pane.read_with(cx, |p, _| p.draft().is_some()),
        "and the draft held for that thread"
    );
    assert_eq!(subjects(&pane, cx), ["two", "one"]);
}

#[gpui_kit::test]
fn a_provider_that_cannot_change_a_draft_locks_the_box_once_it_is_saved(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.without(MailOperation::UpdateDraft);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    write("words", &pane, cx);
    assert!(editable(&pane, cx), "a new draft can be written");
    press("mail-save-draft", &pane, cx);
    assert!(!editable(&pane, cx), "a saved one cannot be changed");
    press("mail-send", &pane, cx);
    assert_eq!(
        provider.sent().len(),
        1,
        "it can still be sent, as it was saved"
    );
}

#[gpui_kit::test]
fn a_thread_starts_at_its_top_and_a_sent_reply_scrolls_to_the_end(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["short"]);
    let long = (0..40)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    provider
        .memory()
        .receive(&atelier_capabilities::mail::Incoming::new(
            "ana@example.com",
            ME,
            "long",
            &long,
            1_790_000_900_000,
        ))
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    // A short window, so the thread is taller than the room it has.
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(420.)));
    settle(&pane, cx);
    let at = |pane: &Entity<MailPane>, cx: &mut VisualTestContext| {
        pane.read_with(cx, |p, _| {
            (
                f32::from(p.messages_scroll().offset().y),
                f32::from(p.messages_scroll().max_offset().y),
            )
        })
    };
    open_row(0, &pane, cx);
    let (offset, max) = at(&pane, cx);
    assert!(
        max > 0. && offset == 0.,
        "the thread is taller than its room and starts at the top: {offset} {max}"
    );
    // Scrolled down, then another thread: it starts at its top again.
    pane.read_with(cx, |p, _| {
        p.messages_scroll()
            .set_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(-80.)))
    });
    open_row(1, &pane, cx);
    assert_eq!(at(&pane, cx).0, 0., "another thread starts at its top");
    open_row(0, &pane, cx);
    write("noted", &pane, cx);
    press("mail-send", &pane, cx);
    settle(&pane, cx);
    let (offset, max) = at(&pane, cx);
    assert!(
        max > 0. && (offset + max).abs() < 1.,
        "the reply that was just sent is in view: {offset} {max}"
    );
}
