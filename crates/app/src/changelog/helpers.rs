include!(concat!(env!("OUT_DIR"), "/releases.rs"));

/// Every release as (version, markdown notes), newest first.
pub fn releases() -> &'static [(&'static str, &'static str)] {
    RELEASES
}

/// The notes of `version`, if the app has them.
pub fn notes_of(version: &str) -> Option<&'static str> {
    RELEASES.iter().find(|(known, _)| *known == version).map(|(_, notes)| *notes)
}
