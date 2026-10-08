use crate::usage_history::structs::{Cache, Day, Roots, UsageHistory};
pub fn read(_r: &Roots, _s: Day, _o: i32) -> UsageHistory { UsageHistory::default() }
pub fn read_cached(_r: &Roots, _s: Day, _o: i32, _c: &mut Cache) -> UsageHistory { UsageHistory::default() }
