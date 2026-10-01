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
