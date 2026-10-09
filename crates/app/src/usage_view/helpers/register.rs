use std::rc::Rc;

use atelier_ui::status_bar::{Press, usage_part};
use gpui_kit::AppContext;

use crate::slots::{Column, RailView, Slots, StatusBarCard};

use super::super::structs::UsagePage;

/// What this module adds to the app: the chips of the limits in the middle of the status bar, and the dashboard they open.
/// With these two registrations gone the bar has no usage chips and the app has no dashboard.
pub fn register(slots: &mut Slots) {
    slots.add_card(StatusBarCard::new(
        "usage",
        Column::Middle,
        20,
        |env, window, cx| {
            let host = env.host.clone();
            let press: Press = Rc::new(move |_, cx| host.open_view("usage", cx));
            usage_part(env.id, env.vitals.providers(), Some(press), window, cx)
                .into_iter()
                .collect()
        },
    ));
    slots.add_view(RailView::new("usage", None, "Usage", 100, |host, cx| {
        let host = host.clone();
        cx.new(|cx| UsagePage::new(host, cx)).into()
    }));
}
