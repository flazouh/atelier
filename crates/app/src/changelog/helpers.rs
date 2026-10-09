include!(concat!(env!("OUT_DIR"), "/releases.rs"));

/// Every release as (version, markdown notes), newest first.
pub fn releases() -> &'static [(&'static str, &'static str)] {
    RELEASES
}

/// The notes of `version`, if the app has them.
pub fn notes_of(version: &str) -> Option<&'static str> {
    RELEASES.iter().find(|(known, _)| *known == version).map(|(_, notes)| *notes)
}

/// What this module adds to the app: the version at the left of the status bar, which opens the notes of the releases.
pub fn register(slots: &mut crate::slots::Slots) {
    use std::rc::Rc;

    use atelier_ui::status_bar::{Press, version_part};

    use crate::slots::{Column, StatusBarCard};

    let version = crate::updater::running_version();
    slots.add_card(StatusBarCard::new("version", Column::Left, 0, move |env, window, cx| {
        let host = env.host.clone();
        let press: Press = Rc::new(move |_, cx| host.show_changelog(cx));
        vec![version_part(env.id, version.clone().into(), Some(press), window, cx)]
    }));
}
