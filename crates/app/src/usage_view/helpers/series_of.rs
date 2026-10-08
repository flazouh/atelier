use atelier_agents::usage_history::Provider;
use atelier_ui::Series;
use crate::usage_view::consts::{CLAUDE_HUE, CODEX_HUE, SHADES};
/// The colour of the `at`-th account of a provider: one hue per provider, one shade per account.
pub fn series_of(provider: Provider, at: usize) -> Series {
    match provider {
        Provider::Claude => Series::new(CLAUDE_HUE, at % SHADES),
        Provider::Codex => Series::new(CODEX_HUE, 0),
    }
}
/// The colour of a model, by its maker.
pub fn model_series(model: &str) -> Series {
    let model = model.to_lowercase();
    if model.contains("gpt") || model.contains("codex") || model.starts_with('o') && model.len() < 4 {
        Series::new(CODEX_HUE, 0)
    } else {
        Series::new(CLAUDE_HUE, 0)
    }
}
