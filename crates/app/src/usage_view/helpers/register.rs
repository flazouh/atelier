use std::rc::Rc;

use atelier_ui::status_bar::{Press, usage_part};

use crate::slots::{Column, Slots, StatusBarCard};

use super::super::structs::UsagePlugin;

/// What this module adds to the app: the chips of the limits in the middle of the status bar, and its plugin, whose view
/// (the dashboard) the chips and the rail open. With this line gone the bar has no usage chips and the app has no dashboard.
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
    slots.plug(&UsagePlugin);
}
