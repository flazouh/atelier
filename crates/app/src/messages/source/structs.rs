use std::sync::Arc;

use atelier_capabilities::{Registry, messaging::MessagingProvider};
use gpui_kit::SharedString;

/// One provider and account, as the sidebar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub provider: String,
    pub account: String,
}

impl Choice {
    /// The provider as a person reads it: `Slack`.
    pub fn name(&self) -> String {
        let mut name = self.provider.chars();
        let head: String = name
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        format!("{head}{}", name.as_str())
    }

    /// The words of its heading: `Slack · acme`.
    pub fn words(&self) -> SharedString {
        format!("{} · {}", self.name(), self.account).into()
    }
}

/// The messaging accounts the screen shows, in the registry's order.
pub struct MessagesSource {
    registry: Registry,
}

impl MessagesSource {
    pub fn from_providers(providers: impl IntoIterator<Item = Arc<dyn MessagingProvider>>) -> Self {
        let mut registry = Registry::new();
        providers
            .into_iter()
            .for_each(|p| registry.add_messaging(p));
        Self { registry }
    }

    /// Every account, in the registry's order: how the sidebar names it, and its provider.
    pub fn accounts(&self) -> Vec<(Choice, Arc<dyn MessagingProvider>)> {
        self.registry
            .all_messaging()
            .into_iter()
            .map(|p| {
                (
                    Choice {
                        provider: p.provider().to_string(),
                        account: p.account().to_string(),
                    },
                    p,
                )
            })
            .collect()
    }

    /// Whether `providers` are the ones held, the very same, in any order. The screen asks each time it opens, and an
    /// answer that has not changed must not read every channel again.
    pub fn same_as(&self, providers: &[Arc<dyn MessagingProvider>]) -> bool {
        let held = self.registry.all_messaging();
        held.len() == providers.len()
            && providers
                .iter()
                .all(|p| held.iter().any(|h| Arc::ptr_eq(h, p)))
    }
}
