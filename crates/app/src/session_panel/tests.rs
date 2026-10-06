use gpui_kit::{TestAppContext, px, size};
use atelier_agents::session::{BlockId, Event};

use super::*;
use crate::fake_agent::{ended, start};

fn text(words: &str) -> Event {
    Event::Text { block: BlockId(0), delta: words.into() }
}

/// View cache: the rows are drawn from their last frame until the session changes. A new turn's rows show
/// in the next frame.
#[gpui_kit::test]
fn cached_rows_show_a_new_turn(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![text("one"), ended()], vec![text("two"), ended()]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("first".into(), cx)));
    cx.run_until_parked();
    let shown = session.clone();
    let (panel, cx) = cx.add_window_view(move |_, cx| SessionPanel::new(shown, cx));
    cx.simulate_resize(size(px(480.), px(800.)));
    cx.run_until_parked();
    let before = cx.update(|_, cx| session.read(cx).shown.len());
    assert!(cx.debug_bounds(Box::leak(format!("row-{}", before - 1).into_boxed_str())).is_some(), "the last row is drawn");
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("second".into(), cx)));
    cx.run_until_parked();
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let after = cx.update(|_, cx| session.read(cx).shown.len());
    assert!(after > before, "the second turn adds rows");
    assert!(
        cx.debug_bounds(Box::leak(format!("row-{}", after - 1).into_boxed_str())).is_some(),
        "the cached rows still end at row {}",
        before - 1
    );
}

/// A session alone in a wide window keeps its rows and its composer to a reading width, centred, with margin on
/// each side; in a narrow panel they take the whole width.
#[gpui_kit::test]
fn a_wide_session_keeps_its_rows_and_composer_to_a_centred_reading_width(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![vec![text("one"), ended()]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("first".into(), cx)));
    cx.run_until_parked();
    let shown = session.clone();
    let (_panel, cx) = cx.add_window_view(move |_, cx| SessionPanel::new(shown, cx));
    let widest = crate::session_view::READING_WIDTH;
    for (window, capped) in [(1600., true), (480., false)] {
        cx.simulate_resize(size(px(window), px(800.)));
        cx.run_until_parked();
        for name in ["entering-0", "session-foot"] {
            let bounds = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
            let (left, width) = (f32::from(bounds.origin.x), f32::from(bounds.size.width));
            let right = window - left - width;
            if capped {
                assert!(width <= widest + 0.5, "{name} at {window}: {width} wide");
                assert!((left - right).abs() < 1., "{name} at {window}: centred, {left} and {right}");
            } else {
                assert!(width > window - 40., "{name} at {window}: takes the width, {width}");
            }
        }
    }
}

/// Where the agent runs is as wide as the account's name and has no chevron; the agent's own picker keeps its width.
#[gpui_kit::test]
fn the_account_picker_fits_its_name(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![], false);
    cx.update(|_, cx| {
        session.update(cx, |s, _| {
            s.provider = Some(crate::providers::Choice::usual());
            s.provider_accounts = vec![atelier_agents::session::Account { name: "default".into(), signed_in: true, plan: Some("max".into()), email: None }];
        })
    });
    let shown = session.clone();
    let (_panel, cx) = cx.add_window_view(move |_, cx| SessionPanel::new(shown, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    let picker = cx.debug_bounds("provider-picker").expect("the account picker is drawn on a new session");
    assert!(picker.size.width < px(120.), "it is as wide as its name, not 180: {picker:?}");
}

/// The panel's header says where the agent runs, as a pill with the account's name; a session with no provider has none.
#[gpui_kit::test]
fn the_header_names_the_provider_in_a_pill(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![], false);
    let shown = session.clone();
    let (_panel, cx) = cx.add_window_view(move |_, cx| SessionPanel::new(shown, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("panel-provider").is_none(), "no provider, no pill");
    cx.update(|_, cx| {
        session.update(cx, |s, cx| {
            s.provider = Some(crate::providers::Choice::OpenRouter);
            cx.notify();
        })
    });
    cx.run_until_parked();
    let pill = cx.debug_bounds("panel-provider").expect("the pill is drawn");
    let title = cx.debug_bounds("panel-project").expect("the project badge is drawn");
    assert!(pill.left() > title.right() && pill.size.height <= px(24.), "in the header row: {pill:?}");
}
