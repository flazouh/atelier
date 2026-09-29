//! The editor's tabs: which files are open, in order, and which one shows. Pure; the view keeps each
//! tab's buffer beside it.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tabs {
    /// Root-relative paths, in tab order.
    open: Vec<String>,
    active: Option<usize>,
}

impl Tabs {
    pub fn paths(&self) -> &[String] {
        &self.open
    }

    pub fn active(&self) -> Option<&str> {
        self.active.map(|i| self.open[i].as_str())
    }

    pub fn position(&self, path: &str) -> Option<usize> {
        self.open.iter().position(|p| p == path)
    }

    /// Shows `path`: its tab when it has one, else a new tab just after the one showing. Returns
    /// whether the tab is new.
    pub fn open(&mut self, path: &str) -> bool {
        if let Some(i) = self.position(path) {
            self.active = Some(i);
            return false;
        }
        let at = self.active.map_or(self.open.len(), |i| i + 1);
        self.open.insert(at, path.to_string());
        self.active = Some(at);
        true
    }

    /// Closes `path`'s tab. The one to its right shows next, or the one to its left at the end.
    pub fn close(&mut self, path: &str) {
        let Some(i) = self.position(path) else { return };
        self.open.remove(i);
        self.active = match self.active {
            _ if self.open.is_empty() => None,
            Some(a) if a > i => Some(a - 1),
            Some(a) if a == i => Some(i.min(self.open.len() - 1)),
            other => other,
        };
    }
}

#[cfg(test)]
mod tests;
