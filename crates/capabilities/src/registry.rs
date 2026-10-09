use std::{collections::BTreeMap, sync::Arc};

use crate::{messaging::MessagingProvider, tasks::TasksProvider};

/// The providers the app has, by capability, provider and account. The screens and the agent tools ask here, and never
/// build a provider themselves.
#[derive(Default)]
pub struct Registry {
    tasks: BTreeMap<(String, String), Arc<dyn TasksProvider>>,
    messaging: BTreeMap<(String, String), Arc<dyn MessagingProvider>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tasks provider. A second one for the same provider and account replaces the first.
    pub fn add_tasks(&mut self, provider: Arc<dyn TasksProvider>) {
        self.tasks.insert(
            (
                provider.provider().to_string(),
                provider.account().to_string(),
            ),
            provider,
        );
    }

    pub fn tasks(&self, provider: &str, account: &str) -> Option<Arc<dyn TasksProvider>> {
        self.tasks
            .get(&(provider.to_string(), account.to_string()))
            .cloned()
    }

    /// Every tasks provider, in provider and account order.
    pub fn all_tasks(&self) -> Vec<Arc<dyn TasksProvider>> {
        self.tasks.values().cloned().collect()
    }

    /// Adds a messaging provider. A second one for the same provider and account replaces the first.
    pub fn add_messaging(&mut self, provider: Arc<dyn MessagingProvider>) {
        let key = (
            provider.provider().to_string(),
            provider.account().to_string(),
        );
        self.messaging.insert(key, provider);
    }

    pub fn messaging(&self, provider: &str, account: &str) -> Option<Arc<dyn MessagingProvider>> {
        self.messaging
            .get(&(provider.to_string(), account.to_string()))
            .cloned()
    }

    /// Every messaging provider, in provider and account order.
    pub fn all_messaging(&self) -> Vec<Arc<dyn MessagingProvider>> {
        self.messaging.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::MemoryTasks;

    #[test]
    fn providers_are_found_by_provider_and_account() {
        let mut r = Registry::new();
        r.add_tasks(Arc::new(MemoryTasks::new("a")));
        r.add_tasks(Arc::new(MemoryTasks::new("b")));
        assert_eq!(r.all_tasks().len(), 2);
        assert_eq!(r.tasks("memory", "b").unwrap().account(), "b");
        assert!(r.tasks("memory", "c").is_none() && r.tasks("linear", "a").is_none());
    }

    #[test]
    fn messaging_providers_are_found_by_provider_and_account() {
        let mut r = Registry::new();
        r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("a")));
        r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("a")));
        assert_eq!(r.all_messaging().len(), 1);
        assert!(r.messaging("memory", "a").is_some() && r.messaging("slack", "a").is_none());
    }

    #[test]
    fn a_second_provider_for_the_same_account_replaces_the_first() {
        let mut r = Registry::new();
        r.add_tasks(Arc::new(MemoryTasks::new("a")));
        r.add_tasks(Arc::new(MemoryTasks::new("a")));
        assert_eq!(r.all_tasks().len(), 1);
    }
}
