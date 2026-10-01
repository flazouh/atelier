use std::hash::{DefaultHasher, Hash, Hasher};

pub(super) fn hash_of(text: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
