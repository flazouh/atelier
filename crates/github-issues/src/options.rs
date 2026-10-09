use std::time::Duration;

use atelier_capabilities::tasks::{Category, Priority};

/// The label names that stand for a status GitHub has no field for. An open issue with none of them is `todo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLabels {
    pub backlog: String,
    pub in_progress: String,
    pub in_review: String,
}

impl Default for StatusLabels {
    fn default() -> Self {
        Self {
            backlog: "status:backlog".into(),
            in_progress: "status:in-progress".into(),
            in_review: "status:in-review".into(),
        }
    }
}

impl StatusLabels {
    /// The label of a category, when it has one.
    pub fn of(&self, category: Category) -> Option<&str> {
        match category {
            Category::Backlog => Some(&self.backlog),
            Category::InProgress => Some(&self.in_progress),
            Category::InReview => Some(&self.in_review),
            Category::Todo | Category::Done | Category::Canceled => None,
        }
    }

    pub(crate) fn all(&self) -> [(Category, &str); 3] {
        [
            (Category::InReview, &self.in_review),
            (Category::InProgress, &self.in_progress),
            (Category::Backlog, &self.backlog),
        ]
    }
}

/// The label names that stand for a priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityLabels {
    pub urgent: String,
    pub high: String,
    pub medium: String,
    pub low: String,
}

impl Default for PriorityLabels {
    fn default() -> Self {
        Self {
            urgent: "priority:urgent".into(),
            high: "priority:high".into(),
            medium: "priority:medium".into(),
            low: "priority:low".into(),
        }
    }
}

impl PriorityLabels {
    pub fn of(&self, priority: Priority) -> Option<&str> {
        match priority {
            Priority::None => None,
            Priority::Urgent => Some(&self.urgent),
            Priority::High => Some(&self.high),
            Priority::Medium => Some(&self.medium),
            Priority::Low => Some(&self.low),
        }
    }

    pub(crate) fn all(&self) -> [(Priority, &str); 4] {
        [
            (Priority::Urgent, &self.urgent),
            (Priority::High, &self.high),
            (Priority::Medium, &self.medium),
            (Priority::Low, &self.low),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub status_labels: StatusLabels,
    pub priority_labels: PriorityLabels,
    /// How often a subscription asks GitHub for changes.
    pub poll_every: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            status_labels: StatusLabels::default(),
            priority_labels: PriorityLabels::default(),
            poll_every: Duration::from_secs(60),
        }
    }
}
