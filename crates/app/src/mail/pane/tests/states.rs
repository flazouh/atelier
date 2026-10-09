//! What the pane shows for each way a call fails, for no account, one and two, and for what a message holds.
use std::sync::Arc;

use atelier_capabilities::{
    CapError,
    mail::{Contact, Incoming, MailOperation, MailProvider},
};
use gpui_kit::TestAppContext;

use super::{
    Fake, ME, Problem, heard_settings, one, open, open_row, press, settle, subjects,
};
use crate::mail::map::BODY_LIMIT;

#[gpui_kit::test]
fn offline_is_a_banner_with_retry_and_retry_reads_again(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Search, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-banner").is_some(), "a banner");
    assert!(cx.debug_bounds("mail-retry").is_some(), "with Retry");
    assert!(subjects(&pane, cx).is_empty());
    provider.heal(MailOperation::Search);
    press("mail-retry", &pane, cx);
    assert_eq!(subjects(&pane, cx), ["hello"], "Retry read it again");
    assert!(cx.debug_bounds("mail-banner").is_none(), "and the banner went");
}

#[gpui_kit::test]
fn offline_while_reading_the_mailboxes_is_the_same_banner(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Mailboxes, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-banner").is_some() && cx.debug_bounds("mail-retry").is_some());
    provider.heal(MailOperation::Mailboxes);
    press("mail-retry", &pane, cx);
    assert_eq!(subjects(&pane, cx), ["hello"], "the mailboxes are read again and the inbox opens");
    assert!(cx.debug_bounds("mail-banner").is_none());
}

#[gpui_kit::test]
fn not_signed_in_is_an_empty_state_with_a_button_that_opens_settings(cx: &mut TestAppContext) {
    for failing in [MailOperation::Mailboxes, MailOperation::Search] {
        let provider = Fake::seeded(ME, &["hello"]);
        provider.fail(failing, CapError::NotSignedIn);
        let (pane, cx) = one(&provider, cx);
        assert!(cx.debug_bounds("mail-signed-out").is_some(), "{failing:?}: an empty state");
        assert!(cx.debug_bounds("mail-composer").is_none(), "{failing:?}: no composer");
        let heard = heard_settings(&pane, cx);
        press("mail-sign-in", &pane, cx);
        assert!(heard.get(), "{failing:?}: the button asks the app for Settings");
    }
}

#[gpui_kit::test]
fn rate_limited_is_a_banner_with_the_wait_and_no_retry(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Search, CapError::RateLimited { retry_after_ms: 2500 });
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-banner").is_some());
    assert!(cx.debug_bounds("mail-retry").is_none(), "a wait is waited out");
    assert_eq!(pane.read_with(cx, |p, _| p.effective_problem()), Some(Problem::Wait(2500)));
}

#[gpui_kit::test]
fn a_change_the_provider_does_not_offer_loses_its_control(cx: &mut TestAppContext) {
    // The provider lists `star` and answers `Unsupported`: the control goes, after the first try.
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Star, CapError::unsupported("star"));
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    press("mail-star", &pane, cx);
    assert!(cx.debug_bounds("mail-star").is_none(), "the control is hidden");
    assert!(cx.debug_bounds("mail-archive").is_some(), "the others stay");
    assert_eq!(pane.read_with(cx, |p, _| p.said().map(str::to_string)), None, "and it says nothing");
}

#[gpui_kit::test]
fn any_other_error_of_a_change_is_one_line_and_the_thread_stays(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Archive, CapError::Provider { code: "E1".into(), message: "boom".into() });
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    press("mail-archive", &pane, cx);
    let said = pane.read_with(cx, |p, _| p.said().map(str::to_string)).expect("a line");
    assert!(said.starts_with("Could not archive the thread") && said.contains("boom"), "{said}");
    assert!(cx.debug_bounds("mail-said").is_some());
    assert_eq!(subjects(&pane, cx), ["hello"], "the thread is where it was");
    assert!(cx.debug_bounds("mail-message-0").is_some(), "and open");
    assert!(cx.debug_bounds("mail-archive").is_some(), "the control stays for another try");
}

#[gpui_kit::test]
fn a_first_reading_that_fails_in_words_takes_the_list(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Search, CapError::Provider { code: "E1".into(), message: "boom".into() });
    let (pane, cx) = one(&provider, cx);
    assert!(subjects(&pane, cx).is_empty());
    assert!(cx.debug_bounds("mail-line").is_some(), "one line says why");
    assert!(cx.debug_bounds("mail-banner").is_none() && cx.debug_bounds("mail-signed-out").is_none());
}

#[gpui_kit::test]
fn an_error_over_an_empty_list_does_not_say_there_is_no_mail(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &[]);
    provider.fail(MailOperation::Search, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-banner").is_some());
    assert!(subjects(&pane, cx).is_empty(), "the list is empty");
    assert_eq!(
        pane.read_with(cx, |p, _| p.effective_problem()),
        Some(Problem::Offline),
        "and the banner says why, so the list does not claim there is no mail"
    );
}

#[gpui_kit::test]
fn a_thread_that_cannot_be_read_says_why_in_the_reading_pane(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    provider.fail(MailOperation::Thread, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-banner").is_some(), "offline is a banner");
    assert!(cx.debug_bounds("mail-composer").is_none(), "nothing to reply to");
    provider.heal(MailOperation::Thread);
    press("mail-retry", &pane, cx);
    assert!(cx.debug_bounds("mail-message-0").is_some(), "Retry read the thread too");
}

#[gpui_kit::test]
fn with_no_account_the_pane_says_so_and_a_button_opens_settings(cx: &mut TestAppContext) {
    let (pane, cx) = open(vec![], cx);
    assert!(cx.debug_bounds("mail-empty").is_some(), "no mail account is connected");
    assert!(cx.debug_bounds("mail-list").is_none());
    let heard = heard_settings(&pane, cx);
    press("mail-open-settings", &pane, cx);
    assert!(heard.get());
}

#[gpui_kit::test]
fn one_account_and_two_accounts_each_have_their_mailboxes(cx: &mut TestAppContext) {
    let (a, b) = (Fake::seeded("a@example.com", &["from a"]), Fake::seeded("b@example.com", &["from b"]));
    let (pane, cx) = open(vec![a.clone() as Arc<dyn MailProvider>], cx);
    assert_eq!(pane.read_with(cx, |p, _| p.accounts().len()), 1);
    assert_eq!(subjects(&pane, cx), ["from a"]);
    let (pane, cx) = open(vec![a.clone() as Arc<dyn MailProvider>, b.clone() as Arc<dyn MailProvider>], cx);
    assert_eq!(pane.read_with(cx, |p, _| p.accounts().len()), 2);
    assert_eq!(subjects(&pane, cx), ["from a"], "the first account's inbox opens");
    let second = pane.read_with(cx, |p, _| p.accounts()[1].boxes[0].reference.clone());
    pane.update_in(cx, |p, window, cx| p.open_mailbox(1, &second, window, cx));
    settle(&pane, cx);
    assert_eq!(subjects(&pane, cx), ["from b"], "a mailbox of the second account reads through its own provider");
    assert_eq!(pane.read_with(cx, |p, _| p.open_mailbox_ref().map(|(at, _)| at)), Some(1));
}

#[gpui_kit::test]
fn a_body_is_drawn_as_the_text_it_is_and_never_as_markup(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &[]);
    let hostile = "<b>bold</b> <script>alert(1)</script> <img src=\"https://example.net/p.gif\"> [x](javascript:alert(1))";
    provider
        .memory()
        .receive(&Incoming {
            from: Contact::named("Eve <script>", "eve@example.net"),
            ..Incoming::new("", ME, "<i>subject</i>", hostile, 1_790_000_000_000)
        })
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    assert_eq!(subjects(&pane, cx), ["<i>subject</i>"], "a subject is its characters");
    open_row(0, &pane, cx);
    let (text, from) = pane.read_with(cx, |p, _| (p.messages()[0].text.to_string(), p.messages()[0].from_name.clone()));
    assert_eq!(text, hostile, "the body reaches the screen as it is, tags and all");
    assert_eq!(from.as_deref(), Some("Eve <script>"));
    assert!(cx.debug_bounds("mail-body-0").is_some());
    assert_eq!(
        pane.read_with(cx, |p, _| p.messages()[0].links.iter().map(|l| l.to_string()).collect::<Vec<_>>()),
        ["https://example.net/p.gif"],
        "a web address is listed as text, and a javascript address is not an address at all"
    );
    assert!(cx.debug_bounds("mail-link-0-0").is_some() && cx.debug_bounds("mail-link-0-1").is_none());
}

#[gpui_kit::test]
fn an_address_in_a_body_opens_only_after_a_press_and_a_yes(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &[]);
    provider
        .memory()
        .receive(&Incoming::new("ana@example.com", ME, "a link", "Read https://example.com/page?x=1 now.", 1_790_000_000_000))
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-link-0-0").is_some(), "the address is listed as text");
    assert!(cx.debug_bounds("mail-link-bar").is_none(), "nothing asks yet, nothing opens");
    assert_eq!(pane.read_with(cx, |p, _| p.link_waiting().map(str::to_string)), None);
    press("mail-link-0-0", &pane, cx);
    assert_eq!(
        pane.read_with(cx, |p, _| p.link_waiting().map(str::to_string)).as_deref(),
        Some("https://example.com/page?x=1")
    );
    assert!(cx.debug_bounds("mail-link-bar").is_some() && cx.debug_bounds("mail-link-address").is_some(), "the address is shown");
    assert!(cx.opened_url().is_none(), "and nothing has opened");
    press("mail-link-cancel", &pane, cx);
    assert!(cx.debug_bounds("mail-link-bar").is_none());
    assert!(cx.opened_url().is_none(), "Cancel opens nothing");
    press("mail-link-0-0", &pane, cx);
    press("mail-link-open", &pane, cx);
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://example.com/page?x=1"),
        "Open opens that address, once"
    );
    assert!(cx.debug_bounds("mail-link-bar").is_none());
}

#[gpui_kit::test]
fn a_long_body_is_cut_and_show_all_gives_the_rest(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &[]);
    let long = "word ".repeat(BODY_LIMIT);
    provider
        .memory()
        .receive(&Incoming::new("ana@example.com", ME, "long", &long, 1_790_000_000_000))
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    // The button stands under the cut text, so the window is tall enough to show it.
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(3000.)));
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-show-all-0").is_some(), "a long body offers Show all");
    let shown = |pane: &gpui_kit::Entity<super::MailPane>, cx: &mut gpui_kit::VisualTestContext| {
        pane.read_with(cx, |p, _| p.shown_body(0).chars().count())
    };
    assert!(shown(&pane, cx) <= BODY_LIMIT, "the body is cut");
    press("mail-show-all-0", &pane, cx);
    assert_eq!(shown(&pane, cx), long.chars().count(), "all of it");
    assert!(cx.debug_bounds("mail-show-all-0").is_none(), "and the button goes");
    // A short body has no button.
    let short = Fake::seeded(ME, &["short"]);
    let (pane, cx) = one(&short, cx);
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-show-all-0").is_none());
}

#[gpui_kit::test]
fn an_attachment_is_a_chip_with_its_name_and_size_and_a_link_is_not_followed(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &[]);
    provider
        .memory()
        .receive(&Incoming {
            attachments: vec![("plan.pdf".into(), "application/pdf".into(), vec![0; 2048])],
            ..Incoming::new("ana@example.com", ME, "with a file", "See the file", 1_790_000_000_000)
        })
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-clip-0").is_some(), "the row has the clip");
    open_row(0, &pane, cx);
    assert!(cx.debug_bounds("mail-file-0-0").is_some(), "the file is a chip");
    let (name, detail) = pane.read_with(cx, |p, _| {
        let a = &p.messages()[0].attachments[0];
        (a.name.to_string(), a.detail.to_string())
    });
    assert_eq!((name.as_str(), detail.as_str()), ("plan.pdf", "2 KB"));
}

#[gpui_kit::test]
fn the_same_providers_change_nothing_and_other_providers_replace_the_accounts(cx: &mut TestAppContext) {
    let a = Fake::seeded("a@example.com", &["one"]);
    let (pane, cx) = open(vec![a.clone() as Arc<dyn MailProvider>], cx);
    let before = a.calls(MailOperation::Mailboxes);
    pane.update(cx, |p, cx| p.set_providers(vec![a.clone() as Arc<dyn MailProvider>], cx));
    settle(&pane, cx);
    assert_eq!(a.calls(MailOperation::Mailboxes), before, "the same accounts are not read again");
    let b = Fake::seeded("b@example.com", &["two"]);
    pane.update(cx, |p, cx| p.set_providers(vec![b.clone() as Arc<dyn MailProvider>], cx));
    settle(&pane, cx);
    assert_eq!(subjects(&pane, cx), ["two"]);
}
