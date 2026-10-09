use std::sync::Arc;
use atelier_agents::usage_history::{AccountUsage, Day, DayTokens, Provider, SessionUsage, Tokens, UsageHistory};
use atelier_agents::labs::Lab;
use atelier_ui::{Gauge, ProviderGauge, Selection, UsageRange, menu::Lead};
use super::super::{helpers::build, structs::UsageState};
const TODAY: Day = Day { year: 2026, month: 10, day: 9 };
fn tokens(input: u64, output: u64) -> Tokens {
    Tokens { input, output, cache_read: 9_000_000, cache_write: 0 }
}
fn session(id: &str, title: &str, model: &str, days: Vec<(Day, u64)>, cost: Option<f64>) -> SessionUsage {
    let days: Vec<DayTokens> = days.into_iter().map(|(day, t)| DayTokens { day, model: model.into(), tokens: tokens(t, 0) }).collect();
    let total: u64 = days.iter().map(|d| d.tokens.input).sum();
    SessionUsage {
        id: id.into(),
        title: title.into(),
        project: "atelier".into(),
        model_main: model.into(),
        days,
        totals: tokens(total, 0),
        est_cost_usd: cost,
        last_active_secs: 0,
    }
}
fn account(provider: Provider, label: &str, sessions: Vec<SessionUsage>) -> AccountUsage {
    AccountUsage { provider, label: label.into(), sessions }
}
/// Two Claude accounts and Codex: the data every test starts from.
fn history() -> UsageHistory {
    UsageHistory {
        accounts: vec![
            account(Provider::Claude, "claude", vec![
                session("a", "Fix the login bug", "claude-opus-5", vec![(TODAY, 1_000_000), (TODAY.minus_days(1), 500_000)], Some(14.2)),
                session("b", "Add tests", "claude-sonnet-5", vec![(TODAY, 200_000)], Some(3.1)),
            ]),
            account(Provider::Claude, "claude-work", vec![session("c", "Landing copy", "mystery-1", vec![(TODAY.minus_days(3), 300_000)], None)]),
            account(Provider::Codex, "codex", vec![session("d", "Refactor", "gpt-5", vec![(TODAY, 400_000)], Some(5.4))]),
        ],
        skipped: 0,
    }
}
fn state(selection: Selection) -> UsageState {
    let mut state = UsageState::new(Arc::new(history()));
    state.selection = selection;
    state.loading = false;
    state
}
fn claude_reading() -> ProviderGauge {
    let mut reading = ProviderGauge::new("Claude", Lead::of(Lab::Anthropic.mark()));
    reading.gauges = vec![
        Gauge { label: "5h".into(), used: 0.21, resets_in: None },
        Gauge { label: "7d".into(), used: 0.45, resets_in: None },
    ];
    reading
}
#[test]
fn every_account_has_a_tile_and_each_claude_account_its_own_shade() {
    let view = build(&state(Selection::All), &[], TODAY);
    let ids: Vec<_> = view.sources.iter().map(|s| s.id.to_string()).collect();
    assert_eq!(ids, ["Claude Code:claude", "Claude Code:claude-work", "Codex:codex"]);
    let shades: Vec<_> = view.sources.iter().map(|s| (s.series.hue, s.series.shade)).collect();
    assert_eq!(shades, [(0, 0), (0, 1), (1, 0)], "one hue for Claude, a shade for each account; Codex has its own hue");
    assert_eq!(view.sources[1].name.as_ref(), "Work");
    assert_eq!(view.sources[0].name.as_ref(), "Default");
}
#[test]
fn the_default_claude_account_carries_the_limit_the_bar_reads_and_the_others_their_tokens() {
    let view = build(&state(Selection::All), &[claude_reading()], TODAY);
    assert_eq!((view.sources[0].value.as_ref(), view.sources[0].note.as_ref()), ("45%", "7d"));
    assert_eq!(view.sources[0].limit, Some(0.45));
    assert_eq!(view.sources[1].limit, None);
    assert_eq!((view.sources[1].value.as_ref(), view.sources[1].note.as_ref()), ("300.0 K", "14d"), "no reading: its own tokens");
}
#[test]
fn the_days_add_up_and_the_last_one_is_today() {
    let view = build(&state(Selection::All), &[], TODAY);
    assert_eq!(view.days.len(), 14);
    let last = view.days.last().unwrap();
    assert_eq!(last.label.as_ref(), "9");
    // Claude default 1.0 M + 0.2 M, Codex 0.4 M: the cache reads are not counted.
    assert_eq!(last.total(), 1_600_000);
    assert!(last.detail.starts_with("Fri 9 · 1.60 M tokens"), "{}", last.detail);
    assert_eq!(view.days[13 - 3].total(), 300_000, "three days ago: the work account");
}
#[test]
fn a_press_on_a_tile_leaves_only_its_account() {
    let view = build(&state(Selection::Source("Codex:codex".into())), &[], TODAY);
    assert_eq!(view.days.last().unwrap().total(), 400_000);
    assert_eq!(view.sessions.len(), 1);
    assert_eq!(view.sessions[0].title.as_ref(), "Refactor");
    assert_eq!(view.models.len(), 1);
}
#[test]
fn a_group_is_all_its_accounts() {
    let view = build(&state(Selection::Group("Claude Code".into())), &[], TODAY);
    assert_eq!(view.sessions.len(), 3);
    assert_eq!(view.days.last().unwrap().total(), 1_200_000);
}
#[test]
fn sessions_come_dearest_first_with_their_days_and_split() {
    let view = build(&state(Selection::All), &[], TODAY);
    let titles: Vec<_> = view.sessions.iter().map(|s| s.title.to_string()).collect();
    assert_eq!(titles.first().map(String::as_str), Some("Fix the login bug"), "the dearest first");
    assert_eq!(titles.last().map(String::as_str), Some("Landing copy"), "a model with no price comes last");
    assert_eq!(titles.len(), 4);
    let priced: Vec<f64> = view.sessions.iter().filter_map(|s| s.cost.trim_start_matches('$').parse().ok()).collect();
    assert!(priced.windows(2).all(|w| w[0] >= w[1]), "the costs only go down: {priced:?}");
    assert_eq!(view.sessions[3].cost.as_ref(), "–");
    let first = &view.sessions[0];
    assert_eq!(first.days.len(), 14);
    assert_eq!((first.days[13], first.days[12]), (1_000_000., 500_000.));
    assert_eq!(first.split[0], ("Cache read".into(), "18.00 M".into()), "the cache read of both days, shown apart");
    assert_eq!(first.footnote.as_ref(), "2 active days");
}
#[test]
fn the_range_changes_what_is_counted() {
    let mut week = state(Selection::All);
    week.range = UsageRange::Week;
    let view = build(&week, &[], TODAY);
    assert_eq!(view.days.len(), 7);
    assert_eq!(view.sessions.len(), 4);
    let mut none = state(Selection::All);
    none.range = UsageRange::Week;
    none.history = Arc::new(UsageHistory { accounts: vec![account(Provider::Claude, "claude", vec![session("z", "Old", "claude-opus-5", vec![(TODAY.minus_days(10), 5)], None)])], skipped: 0 });
    assert!(build(&none, &[], TODAY).empty.is_some(), "nothing in the range: the empty state");
}
#[test]
fn no_logs_say_so_and_a_first_reading_says_it_is_reading() {
    let mut empty = UsageState::new(Arc::new(UsageHistory::default()));
    let reading = build(&empty, &[], TODAY);
    assert_eq!(reading.empty.as_deref(), Some("Reading the session logs…"));
    empty.loading = false;
    assert!(build(&empty, &[], TODAY).empty.unwrap().starts_with("No usage found yet"));
}
#[test]
fn a_key_with_a_limit_has_a_tile_and_no_token_logs() {
    let mut key = ProviderGauge::new("OpenRouter", Lead::of(Lab::OpenRouter.mark())).note("$4.20 of $20 credit");
    key.gauges = vec![Gauge { label: "credit".into(), used: 0.21, resets_in: None }];
    let view = build(&state(Selection::Source("OpenRouter:key".into())), &[key], TODAY);
    let tile = view.sources.last().unwrap();
    assert_eq!((tile.group.as_ref(), tile.value.as_ref(), tile.limit), ("OpenRouter", "21%", Some(0.21)));
    assert!(view.empty.unwrap().contains("no token logs"));
}

#[test]
fn a_redraw_with_the_same_state_gets_the_same_view_and_a_press_a_new_one() {
    let mut state = state(Selection::All);
    let first = build(&state, &[], TODAY);
    assert_eq!(build(&state, &[], TODAY), first, "the same state, the same view");
    state.selection = Selection::Source("Codex:codex".into());
    let second = build(&state, &[], TODAY);
    assert_ne!(second, first, "a press on a tile is not answered with the old view");
    assert_eq!(second.sessions.len(), 1);
    state.range = UsageRange::Week;
    assert_eq!(build(&state, &[], TODAY).days.len(), 7, "nor a change of the range");
    let reading = claude_reading();
    assert_ne!(build(&state, &[reading], TODAY).sources, second.sources, "nor a new reading of a limit");
}
