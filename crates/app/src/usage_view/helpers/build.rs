use std::{collections::BTreeMap, sync::Arc};
use atelier_agents::usage_history::{Day, Provider, Tokens, UsageHistory};
use atelier_ui::{ProviderGauge, Selection, UsageDay, UsageModel, UsageSession, UsageSource, UsageStat};
use gpui_kit::SharedString;
use super::{counted, format_cost, format_tokens, model_series, sources::{group_of, id_of, sources}, weekday};
use crate::usage_view::{
    consts::{MODELS, NOTHING, NO_LOGS, READING, SESSIONS},
    structs::{UsageState, UsageView},
};
/// The accounts a selection stands for, as (provider, label) in the order of the history.
fn chosen<'a>(history: &'a UsageHistory, selection: &Selection) -> Vec<(Provider, &'a str)> {
    history
        .accounts
        .iter()
        .filter(|a| match selection {
            Selection::All => true,
            Selection::Group(caption) => group_of(a.provider) == caption.as_ref(),
            Selection::Source(id) => id_of(a.provider, &a.label) == id.as_ref(),
        })
        .map(|a| (a.provider, a.label.as_str()))
        .collect()
}
fn add(total: &mut Tokens, more: &Tokens) {
    total.input += more.input;
    total.output += more.output;
    total.cache_read += more.cache_read;
    total.cache_write += more.cache_write;
}
fn sum_cost(costs: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    costs.fold(None, |sum, c| match (sum, c) {
        (None, c) => c,
        (Some(a), Some(b)) => Some(a + b),
        (some, None) => some,
    })
}
/// What the dashboard draws, for the state it is in, at `today`. A redraw with the same state and readings gets the view
/// built last time: with thousands of sessions, building it is the one costly step of a frame.
pub fn build(state: &UsageState, readings: &[ProviderGauge], today: Day) -> UsageView {
    let key = format!(
        "{:?}|{:?}|{:?}|{:p}|{}|{readings:?}",
        state.range,
        state.selection,
        today,
        Arc::as_ptr(&state.history),
        state.loading
    );
    if let Some((built_from, view)) = state.built.borrow().as_ref()
        && *built_from == key
    {
        return view.clone();
    }
    let view = build_view(state, readings, today);
    *state.built.borrow_mut() = Some((key, view.clone()));
    view
}
fn build_view(state: &UsageState, readings: &[ProviderGauge], today: Day) -> UsageView {
    let history: &UsageHistory = &state.history;
    let range_days = state.range.days() as u32;
    let sources = sources(history, readings, range_days, today);
    let accounts = chosen(history, &state.selection);
    let by_account = history.days_by_account(range_days, today);
    let at_of = |provider: Provider, label: &str| sources.iter().find(|s| s.id.as_ref() == id_of(provider, label)).map(|s| s.series);
    // The days: one part for each chosen account, bottom first.
    let mut days = Vec::new();
    for i in 0..range_days as usize {
        let day = today.minus_days((range_days as usize - 1 - i) as i64);
        let mut parts = Vec::new();
        let mut cost = Vec::new();
        for (provider, label) in &accounts {
            let Some(series) = at_of(*provider, label) else { continue };
            let Some(entry) = by_account.iter().find(|a| a.provider == *provider && a.label == *label) else { continue };
            if let Some(total) = entry.days.get(i) {
                parts.push((series, counted(&total.tokens)));
                cost.push(total.est_cost_usd);
            }
        }
        let tokens: u64 = parts.iter().map(|(_, t)| t).sum();
        let detail = format!(
            "{} {} · {} tokens · {}",
            weekday(day),
            day.day,
            format_tokens(tokens),
            format_cost(sum_cost(cost.into_iter()))
        );
        days.push(UsageDay { label: day.day.to_string().into(), detail: detail.into(), parts });
    }
    // The models, biggest first, with the tokens of each added over the chosen accounts.
    let mut models: BTreeMap<String, (Tokens, Vec<Option<f64>>)> = BTreeMap::new();
    for (_, label) in &accounts {
        for m in history.models(range_days, today, Some(label)) {
            let entry = models.entry(m.model.clone()).or_default();
            add(&mut entry.0, &m.tokens);
            entry.1.push(m.est_cost_usd);
        }
    }
    let mut models: Vec<_> = models.into_iter().collect();
    models.sort_by_key(|(_, (tokens, _))| std::cmp::Reverse(counted(tokens)));
    let total_tokens: u64 = models.iter().map(|(_, (t, _))| counted(t)).sum();
    let total_cost = sum_cost(models.iter().flat_map(|(_, (_, c))| c.iter().copied()));
    let models: Vec<UsageModel> = models
        .into_iter()
        .take(MODELS)
        .map(|(name, (tokens, _))| UsageModel {
            series: model_series(&name),
            label: format_tokens(counted(&tokens)).into(),
            tokens: counted(&tokens),
            name: name.into(),
        })
        .collect();
    // The sessions, the dearest first.
    let mut ranged = Vec::new();
    for (_, label) in &accounts {
        ranged.extend(history.top_sessions(range_days, today, Some(label), SESSIONS));
    }
    ranged.sort_by(|a, b| {
        b.est_cost_usd.unwrap_or(0.).total_cmp(&a.est_cost_usd.unwrap_or(0.)).then(counted(&b.tokens).cmp(&counted(&a.tokens)))
    });
    ranged.truncate(SESSIONS);
    let sessions: Vec<UsageSession> = ranged
        .iter()
        .map(|s| {
            let series = at_of(s.provider, &s.account).unwrap_or_default();
            let session = history.accounts.iter().filter(|a| a.label == s.account).flat_map(|a| a.sessions.iter()).find(|x| x.id == s.id);
            let per_day: Vec<f32> = (0..range_days as usize)
                .map(|i| {
                    let day = today.minus_days((range_days as usize - 1 - i) as i64);
                    session.map_or(0, |x| x.days.iter().filter(|d| d.day == day).map(|d| counted(&d.tokens)).sum::<u64>()) as f32
                })
                .collect();
            let active = per_day.iter().filter(|v| **v > 0.).count();
            UsageSession {
                id: format!("{}:{}", s.account, s.id).into(),
                title: if s.title.is_empty() { SharedString::from("Untitled session") } else { s.title.clone().into() },
                meta: format!("{} · {} · {}", s.project, group_of(s.provider), s.model_main).into(),
                series,
                tokens: format_tokens(counted(&s.tokens)).into(),
                cost: format_cost(s.est_cost_usd).into(),
                days: per_day,
                split: vec![
                    ("Cache read".into(), format_tokens(s.tokens.cache_read).into()),
                    ("Cache write".into(), format_tokens(s.tokens.cache_write).into()),
                    ("Input".into(), format_tokens(s.tokens.input).into()),
                    ("Output".into(), format_tokens(s.tokens.output).into()),
                ],
                footnote: format!("{active} active {}", if active == 1 { "day" } else { "days" }).into(),
            }
        })
        .collect();
    let today_tokens: u64 = accounts
        .iter()
        .filter_map(|(p, l)| by_account.iter().find(|a| a.provider == *p && a.label == *l))
        .filter_map(|a| a.days.last())
        .map(|d| counted(&d.tokens))
        .sum();
    let nothing = days.iter().all(|d| d.total() == 0);
    let empty = if nothing {
        Some(match (&state.selection, state.loading && history.accounts.is_empty()) {
            (_, true) => READING,
            (Selection::Source(id), _) if id.as_ref().ends_with(":key") => NO_LOGS,
            _ => NOTHING,
        })
    } else {
        None
    };
    let heading = heading(&state.selection, &sources, range_days);
    let total_label = format_tokens(total_tokens);
    let cost_label = format_cost(total_cost);
    let mut stats = Vec::new();
    stats.extend(heading.limit.clone());
    stats.push(UsageStat {
        label: "Today".into(),
        value: format_tokens(today_tokens).into(),
        unit: "tokens".into(),
        note: "Since midnight".into(),
        used: None,
    });
    stats.push(UsageStat {
        label: format!("Last {range_days} days").into(),
        value: total_label.clone().into(),
        unit: "tokens".into(),
        note: format!("{} a day", format_tokens(total_tokens / u64::from(range_days.max(1)))).into(),
        used: None,
    });
    stats.push(UsageStat {
        label: "Estimated cost".into(),
        value: cost_label.clone().into(),
        unit: format!("{range_days} days").into(),
        note: "At API prices, not billed".into(),
        used: None,
    });
    UsageView {
        sources,
        summary: (format_tokens(today_tokens).into(), "today".into()),
        title: heading.title,
        subtitle: heading.subtitle,
        provider: heading.provider,
        dot: heading.dot,
        stats,
        days,
        models,
        sessions,
        total: format!("{total_label} · {cost_label}").into(),
        empty: empty.map(SharedString::from),
    }
}

/// The words over the details and the limit tile, for what is chosen.
struct Heading {
    title: SharedString,
    subtitle: SharedString,
    provider: Option<SharedString>,
    dot: Option<atelier_ui::Series>,
    limit: Option<UsageStat>,
}

fn heading(selection: &Selection, sources: &[UsageSource], range_days: u32) -> Heading {
    match selection {
        Selection::Source(id) => match sources.iter().find(|s| s.id == *id) {
            Some(source) => Heading {
                title: source.name.clone(),
                subtitle: format!(
                    "{} · {}",
                    source.caption,
                    if source.limit.is_some() { format!("{} window", source.note) } else { format!("{range_days} days") }
                )
                .into(),
                provider: Some(source.group.clone()),
                dot: Some(source.series),
                limit: source.limit.map(|used| UsageStat {
                    label: "Limit".into(),
                    value: source.value.clone(),
                    unit: "used".into(),
                    note: source.note.clone(),
                    used: Some(used),
                }),
            },
            None => Heading::all(sources),
        },
        Selection::Group(caption) => {
            let own: Vec<&UsageSource> = sources.iter().filter(|s| s.group == *caption).collect();
            Heading {
                title: caption.clone(),
                subtitle: format!("{} {}", own.len(), if own.len() == 1 { "account" } else { "accounts" }).into(),
                provider: None,
                dot: own.first().map(|s| s.series),
                limit: None,
            }
        }
        Selection::All => Heading::all(sources),
    }
}

impl Heading {
    fn all(sources: &[UsageSource]) -> Self {
        let mut groups: Vec<&str> = Vec::new();
        for source in sources {
            if !groups.contains(&source.group.as_ref()) {
                groups.push(source.group.as_ref());
            }
        }
        let closest = sources
            .iter()
            .filter(|s| s.limit.is_some())
            .max_by(|a, b| a.limit.partial_cmp(&b.limit).unwrap_or(std::cmp::Ordering::Equal));
        Self {
            title: "All accounts".into(),
            subtitle: if sources.is_empty() { "No sources yet".into() } else { format!("{} sources · {}", sources.len(), groups.join(", ")).into() },
            provider: None,
            dot: None,
            limit: closest.map(|s| UsageStat {
                label: "Closest to a limit".into(),
                value: s.value.clone(),
                unit: "used".into(),
                note: format!("{} · {}", s.name.clone(), s.note).into(),
                used: s.limit,
            }),
        }
    }
}
