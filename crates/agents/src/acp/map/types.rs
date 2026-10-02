use std::{
    time::{Instant},
};

use crate::session::BlockId;

pub(super) enum Open {
    Text(BlockId),
    Thinking(BlockId, Instant),
}
