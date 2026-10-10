use std::time::Instant;

use gpui_kit::PathBuilder;

use super::face_set::DATA;
use crate::enums::Mood;
use crate::impls::trace_for_tests as trace;
use crate::structs::{Affine, BotRuntime, FaceSet};

/// The CPU cost of a frame with no window: move every bot, then trace and tessellate every shape. Run it with
/// `cargo test --release -p atelier-bot-face frame_cost -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn frame_cost_of_many_bots() {
    let set = FaceSet::from_json(DATA).unwrap();
    for (count, mood) in [
        (7, Mood::Idle),
        (30, Mood::Idle),
        (30, Mood::Working),
        (30, Mood::Done),
        (60, Mood::Working),
        (120, Mood::Working),
    ] {
        let mut runtimes: Vec<_> = (0..count)
            .map(|i| (i % set.bots.len(), BotRuntime::new(i as u32 + 1, 0.0)))
            .collect();
        let (mut ticks, mut builds, mut shapes) = (Vec::new(), Vec::new(), 0usize);
        for frame in 0..400 {
            let t = 2.0 + frame as f32 / 60.0;
            let started = Instant::now();
            let frames: Vec<_> = runtimes
                .iter_mut()
                .map(|(ix, rt)| rt.tick(&set, &set.bots[*ix], t, mood, Some((0.3, -0.2))))
                .collect();
            ticks.push(started.elapsed().as_secs_f32() * 1000.0);
            let started = Instant::now();
            shapes = 0;
            for ((ix, _), f) in runtimes.iter().zip(&frames) {
                let base = Affine {
                    a: 1.2,
                    b: 0.0,
                    c: 0.0,
                    d: 1.2,
                    e: 10.0,
                    f: 10.0,
                }
                .then(&Affine::of_pose(&f.root, (60.0, 104.0)));
                for (part, pose) in set.bots[*ix].parts.iter().zip(&f.parts) {
                    let m = base.then(&Affine::of_pose(pose, part.pivot));
                    for s in &part.shapes {
                        let mut b = PathBuilder::fill();
                        trace(&mut b, &m, &s.geometry);
                        let _ = b.build();
                        shapes += 1;
                    }
                }
                for (set_ix, eyes) in set.bots[*ix].eyes.iter().enumerate() {
                    if f.eye_weights[set_ix] < 0.003 {
                        continue;
                    }
                    for s in eyes {
                        let mut b = PathBuilder::fill();
                        trace(&mut b, &base, &s.geometry);
                        let _ = b.build();
                        shapes += 1;
                    }
                }
            }
            builds.push(started.elapsed().as_secs_f32() * 1000.0);
        }
        let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let mut sorted = builds.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        println!(
            "{count:>4} bots {mood:?}: {shapes} shapes a frame, move {:.3} ms, trace and tessellate avg {:.2} ms, p99 {:.2} ms, {:.1} us a shape",
            avg(&ticks),
            avg(&builds),
            sorted[sorted.len() * 99 / 100],
            avg(&builds) * 1000.0 / shapes as f32
        );
    }
}
