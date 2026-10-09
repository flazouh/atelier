//! The usage dashboard: account tiles grouped by provider (three Claude Code accounts take three shades of one hue),
//! tokens per day stacked by account with square inner segments and a rounded top, the models, and the sessions, the first
//! one open on its own days. It shows inline here, as the body of a `Modal` made with `flush`; the sample numbers are not
//! real.
use atelier_ui::{
    ActiveTheme, Selection, Series, SourceKind, UsageDashboard, UsageDay, UsageModel, UsageRange, UsageSession, UsageSources, UsageStat, UsageSource,
};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div, px};
fn source(id: &str, name: &str, caption: &str, group: &str, series: Series, limit: Option<f32>, shown: (&str, &str)) -> UsageSource {
    UsageSource {
        id: id.to_string().into(),
        name: name.to_string().into(),
        caption: caption.to_string().into(),
        group: group.to_string().into(),
        series,
        limit,
        value: shown.0.to_string().into(),
        note: shown.1.to_string().into(),
        kind: if limit.is_some() { SourceKind::Subscription } else { SourceKind::Key },
    }
}
/// Fourteen days of sample use, the last the biggest.
fn days() -> Vec<UsageDay> {
    const BASE: [f32; 14] = [0.55, 0.6, 1.0, 0.45, 0.4, 0.5, 0.3, 0.55, 0.62, 0.7, 0.33, 0.38, 0.28, 0.9];
    BASE.iter()
        .enumerate()
        .map(|(d, base)| {
            let part = |share: f32| (share * base * 8_000_000.) as u64;
            UsageDay {
                label: format!("{}", (d + 26) % 30 + 1).into(),
                detail: "Thu 9 · 5.84 M tokens · $18.40".into(),
                parts: vec![
                    (Series::new(0, 0), part(0.36)),
                    (Series::new(0, 1), part(0.17)),
                    (Series::new(0, 2), part(0.06)),
                    (Series::new(1, 0), part(0.2)),
                    (Series::new(2, 0), part(0.08)),
                ],
            }
        })
        .collect()
}
fn model(name: &str, tokens: u64, label: &str, series: Series) -> UsageModel {
    UsageModel { name: name.into(), tokens, series, label: label.into() }
}
fn session(id: &str, title: &str, meta: &str, series: Series, tokens: &str, cost: &str, seed: f32) -> UsageSession {
    UsageSession {
        id: id.into(),
        title: title.into(),
        meta: meta.into(),
        series,
        tokens: tokens.into(),
        cost: cost.into(),
        days: (0..14).map(|i| 0.35 + 0.65 * ((i as f32 * 1.7 + seed).sin() * 0.5 + 0.5)).collect(),
        split: vec![("Cache read".into(), "1.52 M".into()), ("Input".into(), "0.21 M".into()), ("Output".into(), "0.11 M".into())],
        footnote: "46 turns · 3 days".into(),
    }
}
pub fn usage_story(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let claude = "Claude Code · 3 accounts";
    let list = vec![
            source("max-p", "Max", "personal", claude, Series::new(0, 0), Some(0.45), ("45%", "7d")),
            source("max-w", "Max", "work", claude, Series::new(0, 1), Some(0.82), ("82%", "7d")),
            source("team", "Team", "seat", claude, Series::new(0, 2), Some(0.18), ("18%", "7d")),
            source("codex", "Codex", "subscription", "Codex", Series::new(1, 0), Some(0.31), ("31%", "30d")),
            source("or-w", "Work", "key", "OpenRouter · 2 keys", Series::new(2, 0), Some(0.21), ("$4.20", "of $20")),
            source("or-p", "Personal", "key", "OpenRouter · 2 keys", Series::new(2, 0), None, ("$1.10", "month")),
            source("api", "API", "key", "Anthropic API", Series::new(3, 0), Some(0.76), ("$38", "month")),
    ];
    let sidebar = UsageSources::new("usage-story-sources")
        .sources(list.clone())
        .summary("5.84 M", "today")
        .selection(Selection::Source("max-w".into()));
    let stat = |label: &str, value: &str, unit: &str, note: &str, used: Option<f32>| UsageStat {
        label: label.to_string().into(),
        value: value.to_string().into(),
        unit: unit.to_string().into(),
        note: note.to_string().into(),
        used,
    };
    let dashboard = UsageDashboard::new("usage-story")
        .range(UsageRange::Fortnight)
        .title("Max · work")
        .subtitle("Resets in 2 d 8 h · 7-day window · last used 4 min ago")
        .provider("Claude Code")
        .dot(Series::new(0, 1))
        .stats(vec![
            stat("7-day limit", "82%", "used", "Resets in 2 d 8 h", Some(0.82)),
            stat("Today", "2.1 M", "tokens", "Typical day 1.6 M", None),
            stat("Last 14 days", "31.4 M", "tokens", "12 sessions", None),
            stat("Estimated cost", "$96", "14 days", "At API prices, not billed", None),
        ])
        .sources(list)
        .days(days())
        .models(vec![
            model("Opus 5.5", 41_200_000, "41.2 M", Series::new(0, 0)),
            model("Sonnet 5.5", 9_800_000, "9.8 M", Series::new(0, 0)),
            model("GPT-5", 7_100_000, "7.1 M", Series::new(1, 0)),
            model("Kimi K2", 2_000_000, "2.0 M", Series::new(2, 0)),
        ])
        .total("60.1 M · $186")
        .sessions(vec![
            session("s1", "Fix the login bug", "atelier · Claude Code · Max work · Opus 5.5", Series::new(0, 1), "1,840,000", "$14.20", 0.),
            session("s2", "Landing page copy", "ori · Claude Code · Team seat · Sonnet 5.5", Series::new(0, 2), "960,000", "$3.10", 2.),
            session("s3", "Refactor the review crate", "atelier · Codex · GPT-5", Series::new(1, 0), "720,000", "$5.40", 4.),
        ])
        .expanded(Some("s1".into()));
    div().flex().w(px(1280.)).h(px(900.)).bg(theme.background).child(div().w(px(300.)).p(px(8.)).child(sidebar)).child(
        div().flex_1().min_w_0().px(px(24.)).py(px(10.)).child(dashboard),
    )
}
