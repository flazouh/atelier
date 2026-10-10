use atelier_bot_face::FaceSet;
use atelier_bots::{Body, Bot, Colour, FaceChoice, Harness, Provider, Tool, Voice, starter_crew};

use crate::bots_view::helpers::face_of;
use crate::bots_view::consts::FACE_DATA;

#[test]
fn every_starter_has_the_face_named_by_its_id() {
    let set = FaceSet::from_json(FACE_DATA).unwrap();
    for bot in starter_crew() {
        let face = face_of(&set, &bot).expect("a starter has a face");
        assert_eq!(set.bots[face].id, bot.id.as_str(), "{} is drawn as itself", bot.name);
    }
}

#[test]
fn a_bot_with_no_face_of_its_own_borrows_the_starter_with_its_body() {
    let set = FaceSet::from_json(FACE_DATA).unwrap();
    let bot = Bot {
        id: "rex".parse().unwrap(),
        name: "Rex".into(),
        role: "Tester".into(),
        job: String::new(),
        face: FaceChoice { body: Body::Spring, colour: Colour::Red, tool: Tool::Key },
        harness: Harness::Codex,
        provider: Provider::default(),
        skills: vec![],
        tools: vec![],
        voice: Voice::Playful,
        version: 1,
    };
    assert_eq!(face_of(&set, &bot).map(|m| set.bots[m].id.as_str()), Some("olive"), "a spring body is Olive's");
}
