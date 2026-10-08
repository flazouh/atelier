use std::collections::HashMap;

use crate::usage_history::helpers::cost::{cost_usd, sum_cost};
use crate::usage_history::structs::{
    AccountDays, AccountUsage, Day, DayTotal, ModelTotal, RangedSession, Tokens, UsageHistory,
};

impl UsageHistory {
    /// The accounts a query looks at: the one with this label, or all.
    fn selected<'a>(&'a self, account: Option<&'a str>) -> impl Iterator<Item = &'a AccountUsage> {
        self.accounts.iter().filter(move |a| account.is_none_or(|label| a.label == label))
    }

    /// Per day of the last `range_days` days up to `today` (oldest first, empty days included), the tokens of the
    /// chosen account or of all.
    pub fn days(&self, range_days: u32, today: Day, account: Option<&str>) -> Vec<DayTotal> {
        day_totals(self.selected(account), range_days, today)
    }

    /// [`UsageHistory::days`] for each account.
    pub fn days_by_account(&self, range_days: u32, today: Day) -> Vec<AccountDays> {
        self.accounts
            .iter()
            .map(|a| AccountDays {
                provider: a.provider,
                label: a.label.clone(),
                days: day_totals(std::iter::once(a), range_days, today),
            })
            .collect()
    }

    /// Tokens per model over the range, the biggest first.
    pub fn models(&self, range_days: u32, today: Day, account: Option<&str>) -> Vec<ModelTotal> {
        let Some((from, to)) = range(range_days, today) else { return Vec::new() };
        let mut by_model: HashMap<&str, (Tokens, Option<f64>)> = HashMap::new();
        for s in self.selected(account).flat_map(|a| &a.sessions) {
            for d in s.days.iter().filter(|d| d.day >= from && d.day <= to) {
                let slot = by_model.entry(d.model.as_str()).or_insert((Tokens::default(), Some(0.0)));
                slot.0 += d.tokens;
                slot.1 = sum_cost([slot.1, cost_usd(&d.model, &d.tokens)].into_iter());
            }
        }
        let mut out: Vec<ModelTotal> = by_model
            .into_iter()
            .map(|(model, (tokens, est_cost_usd))| ModelTotal { model: model.to_owned(), tokens, est_cost_usd })
            .collect();
        out.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()).then_with(|| a.model.cmp(&b.model)));
        out
    }

    /// The `n` sessions that cost most in the range (estimated cost, a session without a price counts as 0; ties by
    /// tokens), each with only the tokens of the range.
    pub fn top_sessions(&self, range_days: u32, today: Day, account: Option<&str>, n: usize) -> Vec<RangedSession> {
        let Some((from, to)) = range(range_days, today) else { return Vec::new() };
        let mut out = Vec::new();
        for a in self.selected(account) {
            for s in &a.sessions {
                let mut tokens = Tokens::default();
                let mut cost = Some(0.0);
                let mut any = false;
                for d in s.days.iter().filter(|d| d.day >= from && d.day <= to) {
                    any = true;
                    tokens += d.tokens;
                    cost = sum_cost([cost, cost_usd(&d.model, &d.tokens)].into_iter());
                }
                if any {
                    out.push(RangedSession {
                        account: a.label.clone(),
                        provider: a.provider,
                        id: s.id.clone(),
                        title: s.title.clone(),
                        project: s.project.clone(),
                        model_main: s.model_main.clone(),
                        tokens,
                        est_cost_usd: cost,
                        last_active_secs: s.last_active_secs,
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            let (ca, cb) = (a.est_cost_usd.unwrap_or(0.0), b.est_cost_usd.unwrap_or(0.0));
            cb.total_cmp(&ca).then_with(|| b.tokens.total().cmp(&a.tokens.total())).then_with(|| a.id.cmp(&b.id))
        });
        out.truncate(n);
        out
    }
}

/// First and last day of the last `range_days` days ending `today`; `None` for 0 days.
fn range(range_days: u32, today: Day) -> Option<(Day, Day)> {
    (range_days > 0).then(|| (today.minus_days(i64::from(range_days) - 1), today))
}

fn day_totals<'a>(accounts: impl Iterator<Item = &'a AccountUsage>, range_days: u32, today: Day) -> Vec<DayTotal> {
    let Some((from, to)) = range(range_days, today) else { return Vec::new() };
    let mut out: Vec<DayTotal> = (0..i64::from(range_days))
        .map(|i| DayTotal { day: from.plus_days(i), tokens: Tokens::default(), est_cost_usd: Some(0.0) })
        .collect();
    for d in accounts.flat_map(|a| &a.sessions).flat_map(|s| &s.days).filter(|d| d.day >= from && d.day <= to) {
        let Some(slot) = usize::try_from(d.day.epoch_days() - from.epoch_days()).ok().and_then(|i| out.get_mut(i)) else {
            continue;
        };
        slot.tokens += d.tokens;
        slot.est_cost_usd = sum_cost([slot.est_cost_usd, cost_usd(&d.model, &d.tokens)].into_iter());
    }
    out
}
