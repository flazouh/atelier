use std::{
    sync::OnceLock,
    time::{Instant},
};

pub(super) static START: OnceLock<Instant> = OnceLock::new();
