use crate::enums::{Layer, Mood, Paint, Token};
use crate::structs::{FaceSet, Palette};

pub(super) const DATA: &str = include_str!("../../../../docs/bots/faces.v1.json");

#[test]
fn the_real_data_loads_with_eleven_bots() {
    let set = FaceSet::from_json(DATA).expect("faces.v1.json loads");
    assert_eq!(set.bots.len(), 11);
    for bot in &set.bots {
        assert!(
            bot.parts.iter().any(|p| p.layer == Layer::Body),
            "{} has a body",
            bot.id
        );
        assert_eq!(
            bot.eyes.len(),
            Mood::ALL.len(),
            "{} has eyes for every mood",
            bot.id
        );
        assert!(
            bot.eyes.iter().all(|e| !e.is_empty()),
            "{} has no empty eye set",
            bot.id
        );
        assert!(
            bot.parts.iter().all(|p| !p.shapes.is_empty()),
            "{} has no empty part",
            bot.id
        );
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
    assert_eq!(set.bot("dot").map(|b| b.name.as_str()), Some("Dot"));
    assert!(set.bot("nope").is_none());
}

#[test]
fn the_ten_starter_bots_and_keyla_are_all_there_in_the_order_of_the_spec() {
    let set = FaceSet::from_json(DATA).unwrap();
    let names: Vec<_> = set.bots.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Bolt", "Pip", "Olive", "Skip", "Dot", "Nimbus", "Keyla", "Quill", "Ink", "Mimi", "Gus"
        ]
    );
}

#[test]
fn every_bot_uses_the_view_box_of_the_data() {
    let set = FaceSet::from_json(DATA).unwrap();
    assert!(
        set.bots
            .iter()
            .all(|b| b.view == [-4.0, -2.0, 128.0, 128.0])
    );
}

#[test]
fn the_data_has_no_gradient_filter_or_group_the_player_cannot_draw() {
    for word in [
        "url(",
        "filter",
        "<g ",
        "<g>",
        "linearGradient",
        "radialGradient",
    ] {
        assert!(!DATA.contains(word), "faces.v1.json holds `{word}`");
    }
}

#[test]
fn every_bot_has_a_body_part_a_reactor_and_cheeks_or_a_reason() {
    let set = FaceSet::from_json(DATA).unwrap();
    for bot in &set.bots {
        assert!(
            bot.parts.len() >= 3,
            "{} has body, reactor and a way to move or a tool",
            bot.id
        );
        assert!(!bot.cheeks.is_empty(), "{} has cheeks", bot.id);
    }
}

/// The storybook outline is the theme's ink, light on a dark page: no outline in the data names the dark ink itself.
#[test]
fn every_outline_in_the_data_takes_the_ink_token() {
    let set = FaceSet::from_json(DATA).expect("faces.v1.json loads");
    let dark = gpui_kit::Rgba::try_from("#141413").unwrap();
    let mut ink = 0;
    for bot in &set.bots {
        for shape in bot.parts.iter().flat_map(|p| &p.shapes) {
            match shape.stroke.map(|s| s.paint) {
                Some(Paint::Token(Token::Ink)) => ink += 1,
                Some(Paint::Literal(c)) => assert_ne!(c, dark, "{}: an outline names the dark ink instead of {{ink}}", bot.id),
                _ => {}
            }
        }
    }
    assert!(ink > 0, "the outlines take the ink token");
}

#[test]
fn the_standard_palette_draws_the_outline_in_the_dark_ink() {
    let palette = Palette::standard(gpui_kit::Rgba::try_from("#6CCBFA").unwrap());
    assert_eq!(palette.colour(Token::Ink), gpui_kit::rgb(0x141413));
}
