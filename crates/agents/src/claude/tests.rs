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
fn the_spark_thinks_orbits_and_rests_on_the_first_thinking_frame() {
    let mark = mark();
    assert_eq!(mark.working, SparkState::Thinking.strip());
    assert_eq!(mark.orbiting, SparkState::Orbiting.strip());
    assert_eq!(mark.rest, SparkState::Thinking.strip());
}
