use gpui_kit::AssetSource;
use super::*;

#[test]
fn every_claude_strip_is_served_at_its_own_path() {
    for state in claude::SparkState::ALL {
        let strip = state.strip();
        let served = Assets.load(strip.path).unwrap().unwrap_or_else(|| panic!("{} is not served", strip.path));
        assert_eq!(served.as_ref(), strip.bytes, "{state:?}");
    }
}

#[test]
fn what_no_agent_ships_falls_back_to_beui() {
    assert!(Assets.load("icons/bot.svg").unwrap().is_some());
    assert!(strip_bytes("icons/bot.svg").is_none());
}
