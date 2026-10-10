use crate::consts::STILL_T;
use crate::enums::HabitKind;
use crate::impls::habit_pose_for_tests as habit;
use crate::structs::HabitDef;

fn def(kind: HabitKind) -> HabitDef {
    HabitDef {
        kind,
        amp_deg: None,
        every_s: None,
        dx: None,
        dy: None,
        rot_deg: None,
        hz: Some(1.0),
        squash: None,
        amp: Some(0.5),
        deg_per_s: None,
        stretch: None,
        phase: None,
    }
}

#[test]
fn a_rotor_only_ever_narrows_the_part_and_never_widens_it() {
    let h = def(HabitKind::Rotor);
    for i in 0..200 {
        let pose = habit(&h, (60.0, 24.0), STILL_T + i as f32 / 20.0, 1.0, 1.0, 0.0);
        assert!(pose.sx <= 1.0 && pose.sx >= 0.5, "{}", pose.sx);
        assert_eq!(pose.sy, 1.0);
    }
}

#[test]
fn a_pulse_grows_and_shrinks_both_ways_together() {
    let h = def(HabitKind::Pulse);
    let (mut big, mut small) = (1.0f32, 1.0f32);
    for i in 0..200 {
        let pose = habit(&h, (60.0, 86.0), i as f32 / 20.0, 1.0, 1.0, 0.0);
        assert_eq!(pose.sx, pose.sy);
        big = big.max(pose.sx);
        small = small.min(pose.sx);
    }
    assert!(big > 1.4 && small < 0.6, "{small} .. {big}");
}

#[test]
fn a_busy_mood_makes_a_habit_bigger() {
    let h = def(HabitKind::Pulse);
    let calm = habit(&h, (0.0, 0.0), 0.4, 1.0, 0.25, 0.0);
    let busy = habit(&h, (0.0, 0.0), 0.4, 1.0, 1.0, 0.0);
    assert!((busy.sx - 1.0).abs() > (calm.sx - 1.0).abs());
}
