use crate::enums::{Layer, Mood};
use crate::structs::FaceSet;

pub(super) const DATA: &str = include_str!("../../../../docs/bots/faces.v1.json");

#[test]
fn the_real_data_loads_with_seven_bots() {
    let set = FaceSet::from_json(DATA).expect("faces.v1.json loads");
    assert_eq!(set.bots.len(), 7);
    for bot in &set.bots {
        assert!(bot.parts.iter().any(|p| p.layer == Layer::Body), "{} has a body", bot.id);
        assert_eq!(bot.eyes.len(), Mood::ALL.len(), "{} has eyes for every mood", bot.id);
        assert!(bot.eyes.iter().all(|e| !e.is_empty()), "{} has no empty eye set", bot.id);
        assert!(bot.parts.iter().all(|p| !p.shapes.is_empty()), "{} has no empty part", bot.id);
    }
}

#[test]
fn the_states_come_from_the_data() {
    let set = FaceSet::from_json(DATA).unwrap();
    assert_eq!(set.state(Mood::Working).speed, 3.2);
    assert_eq!(set.state(Mood::Stuck).amount, 0.25);
}

#[test]
fn a_wrong_version_is_refused() {
    let bad = DATA.replacen("\"version\": 1", "\"version\": 2", 1);
    assert!(FaceSet::from_json(&bad).unwrap_err().contains("version 2"));
}

#[test]
fn a_bot_is_found_by_id_and_a_missing_one_is_none() {
    let set = FaceSet::from_json(DATA).unwrap();
    assert_eq!(set.bot("dome").map(|b| b.name.as_str()), Some("Dot"));
    assert!(set.bot("nope").is_none());
}
