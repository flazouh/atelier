use crate::usage_history::{AccountUsage, Day, DayTokens, Provider, SessionUsage, Tokens, UsageHistory};

fn t(input: u64, output: u64) -> Tokens {
    Tokens { input, output, cache_read: 0, cache_write: 0 }
}

fn session(id: &str, days: &[(Day, &str, Tokens)]) -> SessionUsage {
    let mut totals = Tokens::default();
    for (_, _, tokens) in days {
        totals += *tokens;
    }
    SessionUsage {
        id: id.to_owned(),
        title: id.to_owned(),
        project: "proj".to_owned(),
        model_main: days[0].1.to_owned(),
        days: days.iter().map(|(day, model, tokens)| DayTokens { day: *day, model: (*model).to_owned(), tokens: *tokens }).collect(),
        totals,
        est_cost_usd: None,
        last_active_secs: 0,
    }
}

const D24: Day = Day { year: 2026, month: 9, day: 24 };
const D25: Day = Day { year: 2026, month: 9, day: 25 };
const D26: Day = Day { year: 2026, month: 9, day: 26 };

fn history() -> UsageHistory {
    UsageHistory {
        skipped: 0,
        accounts: vec![
            AccountUsage {
                provider: Provider::Claude,
                label: "claude".into(),
                sessions: vec![
                    session("a1", &[(D25, "claude-sonnet-4-6", t(1_000_000, 0)), (D26, "claude-opus-5-5", t(1_000_000, 0))]),
                    session("a2", &[(D24, "claude-sonnet-4-6", t(5, 5))]),
                ],
            },
            AccountUsage {
                provider: Provider::Codex,
                label: "codex".into(),
                sessions: vec![session("c1", &[(D26, "gpt-5-codex", t(2_000_000, 0))])],
            },
        ],
    }
}

#[test]
fn days_are_zero_filled_oldest_first_and_sum_the_accounts() {
    let days = history().days(3, D26, None);
    assert_eq!(days.iter().map(|d| d.day).collect::<Vec<_>>(), [D24, D25, D26]);
    assert_eq!(days[0].tokens.total(), 10);
    assert_eq!(days[1].tokens.input, 1_000_000);
    assert_eq!(days[2].tokens.input, 3_000_000);
    // day 26: opus 1M in at $5 plus gpt-5 2M in at $1.25
    assert!((days[2].est_cost_usd.unwrap() - 7.5).abs() < 1e-9);
    let empty = history().days(2, Day::new(2026, 10, 5), None);
    assert!(empty.iter().all(|d| d.tokens.total() == 0 && d.est_cost_usd == Some(0.0)));
    assert!(history().days(0, D26, None).is_empty());
}

#[test]
fn an_account_filter_and_per_account_days() {
    let h = history();
    assert_eq!(h.days(1, D26, Some("codex"))[0].tokens.input, 2_000_000);
    assert_eq!(h.days(1, D26, Some("nobody"))[0].tokens.total(), 0);
    let by = h.days_by_account(1, D26);
    assert_eq!(by.len(), 2);
    assert_eq!((by[0].label.as_str(), by[0].days[0].tokens.input), ("claude", 1_000_000));
}

#[test]
fn models_are_biggest_first_and_follow_the_range() {
    let models = history().models(2, D26, None);
    let names: Vec<_> = models.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(names, ["gpt-5-codex", "claude-opus-5-5", "claude-sonnet-4-6"]);
    assert_eq!(models[2].tokens.input, 1_000_000, "the day before the range is out");
    assert_eq!(history().models(2, D26, Some("claude"))[0].model, "claude-opus-5-5");
}

#[test]
fn top_sessions_rank_by_cost_and_count_only_the_range() {
    let top = history().top_sessions(3, D26, None, 2);
    assert_eq!(top.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["c1", "a1"]);
    assert_eq!(top[1].tokens.input, 2_000_000);
    assert_eq!(top[0].account, "codex");
    let one_day = history().top_sessions(1, D26, Some("claude"), 5);
    assert_eq!(one_day.len(), 1);
    assert_eq!(one_day[0].tokens.input, 1_000_000);
}
