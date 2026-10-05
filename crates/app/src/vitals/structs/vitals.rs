use atelier_ui::{ProviderGauge, SystemLoad, Work};

use super::super::traits::LoadProbe;

/// The last sample of the machine and the last reading of each provider.
pub struct Vitals {
    pub(in super::super) probe: Box<dyn LoadProbe>,
    pub(in super::super) load: Option<SystemLoad>,
    pub(in super::super) providers: Vec<ProviderGauge>,
    pub(in super::super) work: Work,
}
