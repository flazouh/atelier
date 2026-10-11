use atelier_ui::{ProviderGauge, SystemLoad};

/// The numbers of the status bar, as the app last read them: a copy, which does not change after it is taken.
#[derive(Clone, Debug, Default)]
pub struct Vitals {
    /// How hard this machine works; none before the first sample.
    pub load: Option<SystemLoad>,
    /// Each provider's use of its plan.
    pub providers: Vec<ProviderGauge>,
}
