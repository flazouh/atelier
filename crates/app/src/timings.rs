//! `LATHE_TIMINGS=1`: when the first frame showed, and when the first frame with the restored project and
//! session showed, both counted from the start of the process.

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

static START: OnceLock<Instant> = OnceLock::new();

/// Call first thing in `main`.
pub fn mark_start() -> Instant {
    *START.get_or_init(Instant::now)
}

pub fn since_start() -> Duration {
    START.get().map_or(Duration::ZERO, Instant::elapsed)
}

pub fn enabled() -> bool {
    std::env::var("LATHE_TIMINGS").is_ok_and(|v| v == "1")
}
