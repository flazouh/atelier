use std::{
    time::{Duration},
};

use gpui_kit::LayoutId;

use super::types::FRAME_MS;

/// The interval the report counts frames over, from `GALLERY_FRAME_MS`.
pub fn frame_limit() -> Duration {
    let ms = std::env::var("GALLERY_FRAME_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(FRAME_MS);
    Duration::from_secs_f64(ms / 1000.)
}

pub fn report(name: &str, samples: &mut [Duration]) {
    let limit = frame_limit();
    samples.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let at = |q: f64| samples[((samples.len() as f64 * q).ceil() as usize).clamp(1, samples.len()) - 1];
    let over = samples.iter().filter(|d| **d > limit).count();
    println!(
        "{name:<20} median {:>8.3} ms  p95 {:>8.3} ms  max {:>8.3} ms  over {:.2} ms: {over}/{}",
        ms(at(0.5)),
        ms(at(0.95)),
        ms(*samples.last().unwrap()),
        ms(limit),
        samples.len()
    );
}

/// Prints the node count of the first frame. Taffy reuses freed slots after that, so only the first
/// frame, built on a fresh tree, gives the exact count; the largest index of any frame comes next.
pub fn report_count(name: &str, samples: &[Duration]) {
    let count = |d: Duration| d.as_nanos();
    println!("{name:<20} first frame {:>6}  max {:>6}", count(samples[0]), count(*samples.iter().max().unwrap()));
}

/// A layout id's node index, read from its debug form (`LayoutId(NodeId(n))`), for gpui keeps it private.
pub(super) fn node_index(id: LayoutId) -> usize {
    let text = format!("{id:?}");
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().map_or(0, |n| (n & 0xffff_ffff) as usize)
}
