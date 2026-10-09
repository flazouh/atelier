//! The Messages view of the shell: its place on the rail, its sidebar of accounts and channels, and its main pane.
use std::sync::Arc;

use atelier_capabilities::{
    Actor,
    messaging::{ChannelKind, MemoryMessaging, MessagingProvider, NewMessage},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::super::{Shell, ShellView};
use super::{settle, with_a_session};
use crate::capability_hub::CapabilityHub;

/// An app whose hub holds `providers`, as the Accounts section of Settings will leave it.
fn with_accounts(providers: Vec<Arc<dyn MessagingProvider>>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let hub = CapabilityHub::new(Actor::person("me", "me"), None, false);
        providers.into_iter().for_each(|p| hub.add_messaging(p));
        cx.set_global(hub);
    });
}

fn acme() -> Arc<MemoryMessaging> {
    let p = MemoryMessaging::new("acme");
    let general = p.add_channel("general", ChannelKind::Public);
    let design = p.add_channel("design", ChannelKind::Private);
    p.add_channel("Ana", ChannelKind::Dm);
    let ana = Actor::person("ana", "Ana");
    p.send(&NewMessage::to(&general, "welcome to general"), &ana).unwrap();
    p.send(&NewMessage::to(&design, "the design notes"), &ana).unwrap();
    p.set_unread(&design, 2).unwrap();
    Arc::new(p)
}

fn press(shell: &Entity<Shell>, name: &'static str, cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
    cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
    settle(shell, cx);
}

fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    let pane = shell.read_with(cx, |s, _| s.messages.clone().expect("the pane is made"));
    pane.read_with(cx, |p, _| p.lines().iter().map(|l| format!("{}: {}", l.author, match &l.body {
        crate::messages::map::Body::Plain(t) | crate::messages::map::Body::Markdown(t) => t,
    })).collect())
}

#[test]
fn messages_is_a_view_the_settings_can_keep() {
    assert_eq!(ShellView::Messages.words(), "messages");
    assert_eq!(ShellView::from_words(Some("messages")), ShellView::Messages);
    assert!(ShellView::ON_RAIL.contains(&ShellView::Messages));
    assert_eq!(ShellView::Messages.lens(), ShellView::Messages);
}

#[gpui_kit::test]
fn the_rail_ends_with_messages_and_a_press_shows_accounts_channels_and_history(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    with_accounts(vec![acme()], cx);
    let git = cx.debug_bounds("rail-git").unwrap();
    let messages = cx.debug_bounds("rail-messages").expect("Messages is on the rail");
    assert!(git.top() < messages.top(), "after Sessions, Tasks and Code, which keep their places");
    press(&shell, "rail-messages", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Messages);
    assert!(cx.debug_bounds("messages-sidebar").is_some() && cx.debug_bounds("messages-pane").is_some());
    assert!(cx.debug_bounds("messages-account-0").is_some(), "the account has its heading");
    for row in ["messages-channel-0-0", "messages-channel-0-1", "messages-channel-0-2"] {
        assert!(cx.debug_bounds(row).is_some(), "{row} is listed");
    }
    assert!(cx.debug_bounds("messages-unread-0-1").is_some(), "the channel the provider says is unread has its dot");
    assert!(cx.debug_bounds("messages-unread-0-0").is_none() && cx.debug_bounds("messages-unread-0-2").is_none());
    assert_eq!(shown(&shell, cx), ["Ana: welcome to general"], "the first channel is open");
    press(&shell, "messages-channel-0-1", cx);
    assert_eq!(shown(&shell, cx), ["Ana: the design notes"], "a press on a channel opens it");
    assert!(cx.debug_bounds("messages-unread-0-1").is_none(), "and it is read now");
    assert!(cx.debug_bounds("messages-composer").is_some());
}

#[gpui_kit::test]
fn with_no_account_the_view_says_so_and_a_button_opens_settings(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    press(&shell, "rail-messages", cx);
    assert!(cx.debug_bounds("messages-empty").is_some(), "no chat account is connected");
    assert!(shell.read_with(cx, |s, _| s.settings.is_none()));
    press(&shell, "messages-open-settings", cx);
    assert!(shell.read_with(cx, |s, _| s.settings.is_some()), "the button opens Settings");
}

#[gpui_kit::test]
fn an_account_added_after_the_first_look_shows_at_the_next(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    press(&shell, "rail-messages", cx);
    assert!(cx.debug_bounds("messages-empty").is_some());
    press(&shell, "rail-sessions", cx);
    with_accounts(vec![acme()], cx);
    press(&shell, "rail-messages", cx);
    assert!(cx.debug_bounds("messages-empty").is_none() && cx.debug_bounds("messages-channel-0-0").is_some());
}

#[gpui_kit::test]
fn two_accounts_are_two_headings_each_with_its_channels(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let other = MemoryMessaging::new("beta");
    other.add_channel("ops", ChannelKind::Public);
    with_accounts(vec![acme(), Arc::new(other)], cx);
    press(&shell, "rail-messages", cx);
    assert!(cx.debug_bounds("messages-account-0").is_some() && cx.debug_bounds("messages-account-1").is_some());
    assert!(cx.debug_bounds("messages-channel-1-0").is_some());
    press(&shell, "messages-channel-1-0", cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.messages.as_ref().and_then(|p| p.read(cx).open_channel_ref().map(|(at, _)| at))), Some(1));
}

#[gpui_kit::test]
fn a_window_that_closed_on_messages_opens_on_it(cx: &mut TestAppContext) {
    let saved = atelier_settings::Settings { view: Some("messages".into()), ..Default::default() };
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = super::open_shell_with(cx, saved);
    with_accounts(vec![acme()], cx);
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Messages);
    assert_eq!(shown(&shell, cx), ["Ana: welcome to general"], "the pane is made when the view is in front, with no press");
}

/// Zoomed or not, the pane stands a panels' gap from the sidebar's card and 8 px from the window's edge, as the Tasks pane does.
#[gpui_kit::test]
fn the_pane_keeps_its_gaps_at_every_zoom(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 2000.);
    with_accounts(vec![acme()], cx);
    press(&shell, "rail-messages", cx);
    let zoom_in = if cfg!(target_os = "macos") { "cmd-=" } else { "ctrl-=" };
    let near = |a: gpui_kit::Pixels, b: f32| (f32::from(a) - b).abs() < 1.0;
    for presses in [0, 5] {
        for _ in 0..presses {
            cx.simulate_keystrokes(zoom_in);
        }
        settle(&shell, cx);
        settle(&shell, cx);
        let zoom = atelier_ui::scale::zoom();
        assert!(presses == 0 || (zoom - 1.5).abs() < 1e-4, "the zoom key works from the composer too: {zoom}");
        let (sidebar, pane) = (cx.debug_bounds("messages-sidebar").expect("sidebar"), cx.debug_bounds("messages-pane").unwrap_or_else(|| panic!("pane at zoom {zoom}")));
        assert!(near(pane.left() - sidebar.right(), super::super::types::PANE_GAP * zoom), "zoom {zoom}: the gap {:?}", pane.left() - sidebar.right());
        let view = cx.debug_bounds("sessions-view").unwrap();
        assert!(near(view.right() - pane.right(), 8. * zoom), "zoom {zoom}: the right margin {:?}", view.right() - pane.right());
    }
    for _ in 0..5 {
        cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd--" } else { "ctrl--" });
    }
}
