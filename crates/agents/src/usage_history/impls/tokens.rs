use std::ops::{AddAssign, Sub};

use crate::usage_history::structs::Tokens;

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// True when every count is at least the other's.
    pub(crate) fn covers(&self, other: &Tokens) -> bool {
        self.input >= other.input
            && self.output >= other.output
            && self.cache_read >= other.cache_read
            && self.cache_write >= other.cache_write
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, rhs: Tokens) {
        self.input += rhs.input;
        self.output += rhs.output;
        self.cache_read += rhs.cache_read;
        self.cache_write += rhs.cache_write;
    }
}

/// Saturating, count by count.
impl Sub for Tokens {
    type Output = Tokens;
    fn sub(self, rhs: Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(rhs.input),
            output: self.output.saturating_sub(rhs.output),
            cache_read: self.cache_read.saturating_sub(rhs.cache_read),
            cache_write: self.cache_write.saturating_sub(rhs.cache_write),
        }
    }
}
