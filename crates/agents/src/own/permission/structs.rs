use std::collections::HashSet;

/// Tools the reader allowed for good ("Always allow"), by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    always: HashSet<String>,
}

impl Rules {
    pub fn new(always: impl IntoIterator<Item = String>) -> Self {
        Self { always: always.into_iter().collect() }
    }

    pub fn allow_always(&mut self, tool: &str) {
        self.always.insert(tool.to_string());
    }

    pub fn allows(&self, tool: &str) -> bool {
        self.always.contains(tool)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.always.iter().cloned().collect();
        names.sort();
        names
    }
}
