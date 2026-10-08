use crate::usage_history::structs::Cache;

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Files kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many files the last read had to parse (the others came from the cache).
    pub fn parsed_last_read(&self) -> usize {
        self.parsed_last_read
    }
}
