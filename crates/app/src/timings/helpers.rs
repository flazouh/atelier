use std::{
    time::{Duration, Instant},
};

use super::types::START;

/// Call first thing in `main`.
pub fn mark_start() -> Instant {
    *START.get_or_init(Instant::now)
}

pub fn since_start() -> Duration {
    START.get().map_or(Duration::ZERO, Instant::elapsed)
}

pub fn enabled() -> bool {
    std::env::var("ATELIER_TIMINGS").is_ok_and(|v| v == "1")
}
