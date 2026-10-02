use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

/// A hunk's id: what it says, and which of the hunks that say the same thing it is.
pub(super) fn hunk_id(removed: &[&str], added: &[&str], seen: &mut HashMap<u64, usize>) -> String {
    let mut hasher = DefaultHasher::new();
    removed.hash(&mut hasher);
    added.hash(&mut hasher);
    let key = hasher.finish();
    let n = seen.entry(key).or_default();
    *n += 1;
    format!("{key:016x}.{n}")
}
