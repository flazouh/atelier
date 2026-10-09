use atelier_ui::status_bar::{load_parts, work_part};

use crate::slots::{Column, Slots, StatusBarCard};

/// The cards of the numbers this module holds: the agents that wait on the reader, in the middle of the bar, and how hard
/// the machine works, under the right pane.
pub fn register(slots: &mut Slots) {
    slots.add_card(
        StatusBarCard::new("work", Column::Middle, 10, |env, _, cx| {
            work_part(env.vitals.work, cx).into_iter().collect()
        })
        .when(|vitals| vitals.work.needs_you > 0),
    );
    slots.add_card(
        StatusBarCard::new("system", Column::Right, 0, |env, _, cx| {
            env.vitals
                .load
                .iter()
                .flat_map(|load| load_parts(env.id, load, cx))
                .collect()
        })
        .when(|vitals| vitals.load.is_some()),
    );
}
