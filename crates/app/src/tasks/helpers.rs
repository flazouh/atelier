/// The rules the reader left on. The rules act on the local tracker only, until they move to the capability.
/// Reads the settings file, so call it off the UI thread.
pub fn rules() -> atelier_tracker::RuleSet {
    let off = atelier_settings::path().map(|path| atelier_settings::load(&path).task_rules_off).unwrap_or_default();
    atelier_tracker::RuleSet::from_disabled(off.iter().map(String::as_str))
}
