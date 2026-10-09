//! The Mail view of the shell: its place on the rail, its sidebar of accounts and mailboxes, and its main pane.
use std::sync::Arc;

use atelier_capabilities::{
    Actor,
    mail::{Incoming, MailProvider, MemoryMail},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::super::{Shell, ShellView};
use super::{settle, with_a_session};
use crate::capability_hub::CapabilityHub;

/// An app whose hub holds `providers`, as the Accounts section of Settings will leave it.
fn with_accounts(providers: Vec<Arc<dyn MailProvider>>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let hub = CapabilityHub::new(Actor::person("me", "me"), None, false);
        providers.into_iter().for_each(|p| hub.add_mail(p));
        cx.set_global(hub);
    });
}

fn acme() -> Arc<MemoryMail> {
    let mail = MemoryMail::new("me@acme.test");
    mail.receive(&Incoming::new(
        "ana@acme.test",
        "me@acme.test",
        "welcome",
        "hello",
        1_790_000_000_000,
    ))
    .unwrap();
    mail.receive(&Incoming::new(
        "ben@acme.test",
        "me@acme.test",
        "the design notes",
        "notes",
        1_790_000_060_000,
    ))
    .unwrap();
    Arc::new(mail)
}

fn press(shell: &Entity<Shell>, name: &'static str, cx: &mut VisualTestContext) {
    let at = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} is drawn"));
    cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
    settle(shell, cx);
}

fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    let pane = shell.read_with(cx, |s, _| s.mail.clone().expect("the pane is made"));
    pane.read_with(cx, |p, _| {
        p.rows().iter().map(|r| r.subject.to_string()).collect()
    })
}

#[test]
fn mail_is_a_view_the_settings_can_keep() {
    assert_eq!(ShellView::Mail.words(), "mail");
    assert_eq!(ShellView::from_words(Some("mail")), ShellView::Mail);
    assert_eq!(
        ShellView::ON_RAIL.last(),
        Some(&ShellView::Mail),
        "the rail ends with Mail"
    );
    assert_eq!(ShellView::Mail.lens(), ShellView::Mail);
    assert!(!ShellView::Mail.in_code());
}

#[gpui_kit::test]
fn the_rail_ends_with_mail_and_a_press_shows_accounts_mailboxes_and_threads(
    cx: &mut TestAppContext,
) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    with_accounts(vec![acme()], cx);
    let messages = cx
        .debug_bounds("rail-messages")
        .expect("Messages stays on the rail");
    let mail = cx.debug_bounds("rail-mail").expect("Mail is on the rail");
    assert!(
        messages.top() < mail.top(),
        "after Messages, which keeps its place"
    );
    press(&shell, "rail-mail", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Mail);
    assert!(cx.debug_bounds("mail-sidebar").is_some() && cx.debug_bounds("mail-pane").is_some());
    assert!(
        cx.debug_bounds("mail-account-0").is_some(),
        "the account has its heading"
    );
    for row in [
        "mail-box-0-0",
        "mail-box-0-1",
        "mail-box-0-2",
        "mail-box-0-6",
    ] {
        assert!(cx.debug_bounds(row).is_some(), "{row} is listed");
    }
    assert_eq!(
        shown(&shell, cx),
        ["the design notes", "welcome"],
        "the inbox is open, newest first"
    );
    assert!(cx.debug_bounds("mail-list").is_some() && cx.debug_bounds("mail-reading").is_some());
    // The inbox counts what is unread; a press on another mailbox opens it.
    press(&shell, "mail-box-0-1", cx);
    assert!(shown(&shell, cx).is_empty(), "the sent box is empty");
    press(&shell, "mail-box-0-0", cx);
    assert_eq!(shown(&shell, cx).len(), 2);
}

#[gpui_kit::test]
fn with_no_account_the_view_says_so_and_a_button_opens_settings(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    press(&shell, "rail-mail", cx);
    assert!(
        cx.debug_bounds("mail-empty").is_some(),
        "no mail account is connected"
    );
    assert!(shell.read_with(cx, |s, _| s.settings.is_none()));
    press(&shell, "mail-open-settings", cx);
    assert!(
        shell.read_with(cx, |s, _| s.settings.is_some()),
        "the button opens Settings"
    );
}

#[gpui_kit::test]
fn an_account_added_after_the_first_look_shows_at_the_next(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    press(&shell, "rail-mail", cx);
    assert!(cx.debug_bounds("mail-empty").is_some());
    press(&shell, "rail-sessions", cx);
    with_accounts(vec![acme()], cx);
    press(&shell, "rail-mail", cx);
    assert!(cx.debug_bounds("mail-empty").is_none() && cx.debug_bounds("mail-box-0-0").is_some());
}

#[gpui_kit::test]
fn two_accounts_are_two_headings_each_with_its_mailboxes(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    with_accounts(vec![acme(), Arc::new(MemoryMail::new("me@beta.test"))], cx);
    press(&shell, "rail-mail", cx);
    assert!(
        cx.debug_bounds("mail-account-0").is_some() && cx.debug_bounds("mail-account-1").is_some()
    );
    assert!(cx.debug_bounds("mail-box-1-0").is_some());
    press(&shell, "mail-box-1-0", cx);
    assert_eq!(
        shell.read_with(cx, |s, cx| s
            .mail
            .as_ref()
            .and_then(|p| p.read(cx).open_mailbox_ref().map(|(at, _)| at))),
        Some(1)
    );
}

#[gpui_kit::test]
fn a_window_that_closed_on_mail_opens_on_it(cx: &mut TestAppContext) {
    let saved = atelier_settings::Settings {
        view: Some("mail".into()),
        ..Default::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = super::open_shell_with(cx, saved);
    with_accounts(vec![acme()], cx);
    shell.update_in(cx, |s, window, cx| {
        s.open_local(dir.path().to_path_buf(), window, cx)
    });
    settle(&shell, cx);
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Mail);
    assert_eq!(
        shown(&shell, cx),
        ["the design notes", "welcome"],
        "the pane is made when the view is in front, with no press"
    );
}

/// Zoomed or not, the pane stands a panels' gap from the sidebar's card and 8 px from the window's edge, and its two cards stand
/// a panels' gap from each other, as the other screens' panes do.
#[gpui_kit::test]
fn the_pane_keeps_its_gaps_at_every_zoom(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 2000.);
    with_accounts(vec![acme()], cx);
    press(&shell, "rail-mail", cx);
    let zoom_in = if cfg!(target_os = "macos") {
        "cmd-="
    } else {
        "ctrl-="
    };
    let near = |a: gpui_kit::Pixels, b: f32| (f32::from(a) - b).abs() < 1.0;
    for presses in [0, 5] {
        for _ in 0..presses {
            cx.simulate_keystrokes(zoom_in);
        }
        settle(&shell, cx);
        settle(&shell, cx);
        let zoom = atelier_ui::scale::zoom();
        assert!(
            presses == 0 || (zoom - 1.5).abs() < 1e-4,
            "the zoom key works from the pane too: {zoom}"
        );
        let (sidebar, pane) = (
            cx.debug_bounds("mail-sidebar").expect("sidebar"),
            cx.debug_bounds("mail-pane")
                .unwrap_or_else(|| panic!("pane at zoom {zoom}")),
        );
        assert!(
            near(
                pane.left() - sidebar.right(),
                super::super::types::PANE_GAP * zoom
            ),
            "zoom {zoom}: the gap {:?}",
            pane.left() - sidebar.right()
        );
        let view = cx.debug_bounds("sessions-view").unwrap();
        assert!(
            near(view.right() - pane.right(), 8. * zoom),
            "zoom {zoom}: the right margin {:?}",
            view.right() - pane.right()
        );
        let (list, reading) = (
            cx.debug_bounds("mail-list").expect("list"),
            cx.debug_bounds("mail-reading").expect("reading"),
        );
        assert!(
            near(
                reading.left() - list.right(),
                atelier_ui::panel_layout::GAP * zoom
            ),
            "zoom {zoom}: the gap between the cards {:?}",
            reading.left() - list.right()
        );
        assert!(
            near(list.left() - pane.left(), 0.),
            "zoom {zoom}: the list starts at the pane's edge"
        );
        assert!(
            near(pane.right() - reading.right(), 0.),
            "zoom {zoom}: the reading card ends at the pane's edge"
        );
    }
    for _ in 0..5 {
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd--"
        } else {
            "ctrl--"
        });
    }
}
