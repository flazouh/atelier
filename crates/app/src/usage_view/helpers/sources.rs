use atelier_agents::usage_history::{Day, Provider, UsageHistory};
use atelier_ui::{Gauge, ProviderGauge, Series, SourceKind, UsageSource};
use gpui_kit::SharedString;
use super::{counted, format_tokens, series_of};
use crate::usage_view::consts::OPENROUTER_HUE;
/// The caption over a provider's tiles.
pub fn group_of(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "Claude Code",
        Provider::Codex => "Codex",
    }
}
/// The id of an account's tile.
pub fn id_of(provider: Provider, label: &str) -> String {
    format!("{}:{label}", group_of(provider))
}
/// What an account is called on its tile: the folder name without the prefix of its provider.
fn name_of(provider: Provider, label: &str) -> String {
    let rest = label.trim_start_matches("claude").trim_start_matches("codex").trim_start_matches(['-', '_']);
    if rest.is_empty() {
        return match provider {
            Provider::Claude => "Default".into(),
            Provider::Codex => "Codex".into(),
        };
    }
    let mut chars = rest.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}
/// The longest window of a provider's reading: the one a plan is judged by (`7d`, `30d`).
fn longest(gauges: &[Gauge]) -> Option<&Gauge> {
    gauges.last()
}
fn reading<'a>(readings: &'a [ProviderGauge], name: &str) -> Option<&'a ProviderGauge> {
    readings.iter().find(|r| r.name.as_ref().eq_ignore_ascii_case(name))
}
/// One tile for each account that has logs, then one for a key whose limit the bar reads. The default Claude account
/// and Codex carry the limits the bar reads; an account with no reading shows its tokens in the range.
pub fn sources(history: &UsageHistory, readings: &[ProviderGauge], range_days: u32, today: Day) -> Vec<UsageSource> {
    let mut sources = Vec::new();
    let mut claude_at = 0;
    let default_claude = history
        .accounts
        .iter()
        .find(|a| a.provider == Provider::Claude && a.label == "claude")
        .or_else(|| history.accounts.iter().find(|a| a.provider == Provider::Claude))
        .map(|a| a.label.clone());
    for account in &history.accounts {
        let at = if account.provider == Provider::Claude {
            claude_at += 1;
            claude_at - 1
        } else {
            0
        };
        let provider_reading = match account.provider {
            Provider::Claude if Some(&account.label) == default_claude.as_ref() => reading(readings, "Claude"),
            Provider::Claude => None,
            Provider::Codex => reading(readings, "Codex"),
        };
        let tokens: u64 = history.days(range_days, today, Some(&account.label)).iter().map(|d| counted(&d.tokens)).sum();
        let gauge = provider_reading.and_then(|r| longest(&r.gauges));
        let (value, note, limit) = match gauge {
            Some(g) => (format!("{}%", (g.used.clamp(0., 1.) * 100.).round() as u32), g.label.to_string(), Some(g.used)),
            None => (format_tokens(tokens), format!("{range_days}d"), None),
        };
        sources.push(UsageSource {
            id: id_of(account.provider, &account.label).into(),
            name: name_of(account.provider, &account.label).into(),
            caption: format!("{} sessions", account.sessions.len()).into(),
            group: group_of(account.provider).into(),
            series: series_of(account.provider, at),
            limit,
            value: value.into(),
            note: note.into(),
            kind: SourceKind::Subscription,
        });
    }
    if let Some(open_router) = reading(readings, "OpenRouter") {
        let gauge = longest(&open_router.gauges);
        sources.push(UsageSource {
            id: "OpenRouter:key".into(),
            name: "Key".into(),
            caption: open_router.note.clone().unwrap_or_else(|| SharedString::from("API key")),
            group: "OpenRouter".into(),
            series: Series::new(OPENROUTER_HUE, 0),
            limit: gauge.map(|g| g.used),
            value: gauge.map_or("–".to_string(), |g| format!("{}%", (g.used.clamp(0., 1.) * 100.).round() as u32)).into(),
            note: gauge.map_or(SharedString::from("credit"), |g| g.label.clone()),
            kind: SourceKind::Key,
        });
    }
    sources
}
