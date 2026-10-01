use std::{
    io::{self},
    };

use crate::protocol::{Failure, Reply};
use super::structs::Connection;

/// Makes a new connection: the first one, and each one after a drop.
pub type Dial = Box<dyn Fn() -> io::Result<Connection> + Send + Sync>;

pub(super) type Answer = Result<Reply, Failure>;
