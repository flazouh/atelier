use super::*;

#[test]
fn claudes_look_uses_the_clis_clay_and_names_claude() {
    let look = look();
    assert_eq!(look.message, color(MESSAGE_CLAY));
    assert_eq!(look.glimmer, color(GLIMMER_CLAY));
    assert_eq!(look.mark.color, color(CLAY));
    assert_eq!(look.labels.waiting, "Waiting for Claude…");
}

#[test]
fn the_spark_thinks_while_claude_works_and_orbits_while_subagents_run() {
    let mark = mark();
    assert_eq!(mark.working, SparkState::Thinking.strip());
    assert_eq!(mark.orbiting, SparkState::Orbiting.strip());
}
