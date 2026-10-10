use super::structs::Slots;

/// The slots as the app starts: each module that has something to add registers it here, and only here. Taking a line out
/// takes that module's cards and views out of the app.
pub fn builtin() -> Slots {
    let mut slots = Slots::default();
    crate::changelog::register(&mut slots);
    crate::vitals::register(&mut slots);
    crate::usage_view::register(&mut slots);
    crate::bots_view::register(&mut slots);
    slots
}
