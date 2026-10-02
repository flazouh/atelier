use super::*;

#[test]
fn states_match_the_app_table() {
    let table = [
        (SparkState::Thinking, 9, 90, true),
        (SparkState::Writing, 8, 90, true),
        (SparkState::Waiting, 16, 600, true),
        (SparkState::Shimmer, 15, 100, true),
        (SparkState::Orbiting, 18, 100, true),
        (SparkState::Exit, 6, 70, false),
    ];
    for (state, frames, ms, loops) in table {
        let strip = state.strip();
        assert_eq!((strip.frames, strip.frame_ms, strip.loops), (frames, ms, loops), "{state:?}");
    }
}

#[test]
fn frame_zero_shows_at_the_start() {
    for state in SparkState::ALL {
        assert_eq!(state.strip().frame_at(0), 0, "{state:?}");
    }
}

#[test]
fn one_shots_hold_their_last_frame() {
    assert_eq!(SparkState::Exit.strip().frame_at(10_000), 5);
}

#[test]
fn each_strip_is_about_one_hundred_units_per_frame() {
    // Exit is a unit or two off; every other strip is exactly 100 x frames*100.
    for state in SparkState::ALL {
        let strip = state.strip();
        let (w, h) = strip.native_size();
        assert!((100. ..=101.).contains(&w), "{state:?} {w}");
        assert!((h - 100. * strip.frames as f32).abs() <= 1., "{state:?} {h}");
    }
}

#[test]
fn each_state_has_its_own_path() {
    let mut paths: Vec<_> = SparkState::ALL.iter().map(|s| s.strip().path).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), SparkState::ALL.len());
    assert!(paths.iter().all(|p| p.starts_with("claude/spark/")));
}
