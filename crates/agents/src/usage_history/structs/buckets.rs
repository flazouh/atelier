use std::collections::{BTreeMap, HashMap};

use super::Tokens;

/// Tokens by model and 15 minute bucket while a file is read.
#[derive(Default)]
pub(crate) struct Buckets {
    pub(crate) by_model: HashMap<String, BTreeMap<i64, Tokens>>,
}
