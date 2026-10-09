use std::sync::Arc;

use atelier_capabilities::{Registry, mail::MailProvider};
use gpui_kit::SharedString;

/// One provider and account, as the sidebar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub provider: String,
    pub account: String,
}

impl Choice {
    /// The provider as a person reads it: `Gmail`.
    pub fn name(&self) -> String {
        let mut name = self.provider.chars();
        let head: String = name
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        format!("{head}{}", name.as_str())
    }

    /// The words of its heading: `Gmail · alex@example.com`.
    pub fn words(&self) -> SharedString {
        format!("{} · {}", self.name(), self.account).into()
    }
}

/// The mail accounts the screen shows, in the registry's order.
pub struct MailSource {
    registry: Registry,
}

impl MailSource {
    pub fn from_providers(providers: impl IntoIterator<Item = Arc<dyn MailProvider>>) -> Self {
        let mut registry = Registry::new();
        providers.into_iter().for_each(|p| registry.add_mail(p));
        Self { registry }
    }

    /// Every account, in the registry's order: how the sidebar names it, and its provider.
    pub fn accounts(&self) -> Vec<(Choice, Arc<dyn MailProvider>)> {
        self.registry
            .all_mail()
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
    /// answer that has not changed must not read every mailbox again.
    pub fn same_as(&self, providers: &[Arc<dyn MailProvider>]) -> bool {
        let held = self.registry.all_mail();
        held.len() == providers.len()
            && providers
                .iter()
                .all(|p| held.iter().any(|h| Arc::ptr_eq(h, p)))
    }
}
