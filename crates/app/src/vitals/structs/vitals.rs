use atelier_ui::{ProviderGauge, SystemLoad, Work};

use gpui_kit::WeakEntity;
use super::super::traits::LoadProbe;
use crate::shell::Shell;

/// The last sample of the machine and the last reading of each provider.
pub struct Vitals {
    pub(in super::super) probe: Box<dyn LoadProbe>,
    pub(in super::super) load: Option<SystemLoad>,
    pub(in super::super) providers: Vec<ProviderGauge>,
    pub(in super::super) work: Work,
    /// The widths of the bar's cards under the sidebar and under the right pane, as the shell lays the columns out.
    pub(in super::super) columns: (Option<f32>, Option<f32>),
    /// The shell that shows the bar, which the cards ask to open a view or the changelog.
    pub(in super::super) shell: WeakEntity<Shell>,
}
