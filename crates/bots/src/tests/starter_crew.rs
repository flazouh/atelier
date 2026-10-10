use std::collections::HashSet;

use crate::{Colour, Harness, starter_crew};

#[test]
fn ten_bots_ship_with_their_own_ids_and_names_and_no_problem() {
    let crew = starter_crew();
    assert_eq!(crew.len(), 10);
    assert_eq!(
        crew.iter()
            .map(|b| b.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        10
    );
    assert_eq!(
        crew.iter()
            .map(|b| b.name.as_str())
            .collect::<HashSet<_>>()
            .len(),
        10
    );
    for bot in &crew {
        assert!(bot.problems().is_empty(), "{:?}", bot.problems());
    }
}

#[test]
fn the_spec_names_are_the_names() {
    let names: Vec<_> = starter_crew().into_iter().map(|b| b.name).collect();
    assert_eq!(
        names,
        [
            "Nimbus", "Bolt", "Pip", "Olive", "Skip", "Dot", "Quill", "Ink", "Mimi", "Gus"
        ]
    );
}

#[test]
fn the_first_six_each_have_a_face_of_their_own() {
    let crew = starter_crew();
    let faces: HashSet<_> = crew
        .iter()
        .take(6)
        .map(|b| (b.face.body as u8, b.face.tool as u8))
        .collect();
    assert_eq!(faces.len(), 6, "six bodies and tools, no two the same");
}

#[test]
fn the_reviewer_runs_on_codex_and_the_rest_on_claude_code() {
    let crew = starter_crew();
    assert_eq!(
        crew.iter().find(|b| b.role == "Reviewer").unwrap().harness,
        Harness::Codex
    );
    assert_eq!(
        crew.iter()
            .filter(|b| b.harness == Harness::ClaudeCode)
            .count(),
        9
    );
}

#[test]
fn all_seven_brand_colours_are_used() {
    let colours: HashSet<_> = starter_crew().iter().map(|b| b.face.colour as u8).collect();
    assert!(colours.contains(&(Colour::Pink as u8)) && colours.len() >= 6);
}

#[test]
fn the_roles_match_the_ten_in_the_spec() {
    let roles: Vec<_> = starter_crew().into_iter().map(|b| b.role).collect();
    assert_eq!(
        roles,
        [
            "Planner",
            "Builder",
            "Prover",
            "Reviewer",
            "Shipper",
            "Debugger",
            "Researcher",
            "Writer",
            "Designer",
            "Operator"
        ]
    );
}
