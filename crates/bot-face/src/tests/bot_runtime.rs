use super::face_set::DATA;
use crate::enums::Mood;
use crate::structs::{BotRuntime, FaceSet};

fn run(mood: Mood, seconds: f32) -> (BotRuntime, crate::structs::Frame, FaceSet) {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(1, 0.0);
    let mut frame = None;
    let mut t = 0.0;
    while t < seconds {
        frame = Some(rt.tick(&set, &set.bots[0], t, mood, None));
        t += 1.0 / 60.0;
    }
    (rt, frame.unwrap(), set)
}

#[test]
fn a_new_mood_takes_over_the_eyes_in_about_a_second() {
    let (_, frame, _) = run(Mood::Working, 1.0);
    assert!(
        frame.eye_weights[Mood::Working.index()] > 0.95,
        "{:?}",
        frame.eye_weights
    );
    assert!(frame.eye_weights[Mood::Idle.index()] < 0.05);
}

#[test]
fn the_weights_always_add_up_to_one() {
    for mood in Mood::ALL {
        let (_, frame, _) = run(mood, 0.4);
        let sum: f32 = frame.eye_weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "{mood:?} {sum}");
    }
}

#[test]
fn a_done_bot_hops_and_never_sinks_below_the_ground() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(2, 0.0);
    let (mut top, mut bottom) = (0.0f32, 0.0f32);
    let mut t = 0.0;
    while t < 4.0 {
        let f = rt.tick(&set, &set.bots[1], t, Mood::Done, None);
        if t > 1.5 {
            top = top.min(f.root.y);
            bottom = bottom.max(f.root.y);
        }
        t += 1.0 / 60.0;
    }
    assert!(top < -8.0, "it rises about 10 units: {top}");
    assert!(bottom <= 0.01, "it never goes below its start: {bottom}");
}

#[test]
fn a_blink_closes_the_eyes_and_opens_them_again() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(3, 0.0);
    let (mut least, mut t) = (1.0f32, 0.0);
    while t < 8.0 {
        least = least.min(rt.tick(&set, &set.bots[2], t, Mood::Idle, None).blink);
        t += 1.0 / 60.0;
    }
    assert!(least < 0.2, "it blinked: {least}");
    assert!(rt.tick(&set, &set.bots[2], t + 1.0, Mood::Idle, None).blink > 0.99);
}

#[test]
fn a_click_makes_one_jump_and_then_stops() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(4, 0.0);
    for i in 0..120 {
        rt.tick(&set, &set.bots[0], i as f32 / 60.0, Mood::Idle, None);
    }
    rt.react(2.0);
    let mut highest = 0.0f32;
    let mut t = 2.0;
    while t < 3.2 {
        highest = highest.min(rt.tick(&set, &set.bots[0], t, Mood::Idle, None).root.y);
        t += 1.0 / 60.0;
    }
    assert!(
        highest < -12.0,
        "the jump reaches about 18 units: {highest}"
    );
    assert!(rt.react_at.is_none(), "the reaction ends");
}

#[test]
fn a_bot_grows_in_after_its_delay() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(5, 0.5);
    assert!(
        rt.tick(&set, &set.bots[0], 0.1, Mood::Idle, None)
            .root
            .sx
            .abs()
            < 1e-3
    );
    assert!((rt.tick(&set, &set.bots[0], 2.0, Mood::Idle, None).root.sx - 1.0).abs() < 0.05);
}

#[test]
fn the_eyes_look_toward_the_pointer_but_not_when_stuck() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut idle = BotRuntime::new(6, 0.0);
    let mut stuck = BotRuntime::new(6, 0.0);
    let (mut a, mut b) = (None, None);
    for i in 0..120 {
        let t = 1.0 + i as f32 / 60.0;
        a = Some(idle.tick(&set, &set.bots[0], t, Mood::Idle, Some((1.0, -1.0))));
        b = Some(stuck.tick(&set, &set.bots[0], t, Mood::Stuck, Some((1.0, -1.0))));
    }
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(
        (a.look.0 - 3.2).abs() < 0.01 && (a.look.1 + 2.2).abs() < 0.01,
        "{:?}",
        a.look
    );
    assert!(b.look.0.abs() < 0.1, "{:?}", b.look);
}

#[test]
fn a_still_frame_shows_the_mood_at_once_and_does_not_move() {
    let set = FaceSet::from_json(DATA).unwrap();
    let mut rt = BotRuntime::new(7, 3.0);
    rt.react(0.5);
    let a = rt.still(&set, set.bot("skip").unwrap(), Mood::Needs);
    let b = rt.still(&set, set.bot("skip").unwrap(), Mood::Needs);
    assert_eq!(a, b, "two still frames are the same");
    assert_eq!(a.eye_weights[Mood::Needs.index()], 1.0);
    assert_eq!(a.blink, 1.0);
    assert!(
        (a.root.sx - 1.0).abs() < 0.2,
        "full size, not growing in: {:?}",
        a.root
    );
    assert_eq!(a.scan_x, 0.0);
}
