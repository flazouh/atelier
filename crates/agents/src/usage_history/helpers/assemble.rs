use std::collections::BTreeMap;
use std::sync::Arc;

use super::cost::{cost_usd, sum_cost};
use super::project_name::project_name;
use crate::usage_history::consts::{BUCKET_SECS, UNTITLED};
use crate::usage_history::structs::{AccountUsage, Day, DayTokens, FileUsage, Roots, SessionUsage, Tokens};

/// Builds the accounts from parsed files, keeping only the days from `since` on. A subagent file counts for its
/// parent session.
pub(crate) fn assemble(roots: &Roots, files: &[(usize, Arc<FileUsage>)], since: Day, offset_secs: i32) -> Vec<AccountUsage> {
    roots
        .accounts()
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let mut by_session: BTreeMap<&str, Vec<&FileUsage>> = BTreeMap::new();
            for (account, file) in files {
                if *account == index {
                    by_session.entry(file.session_id.as_str()).or_default().push(file);
                }
            }
            let mut sessions: Vec<SessionUsage> = by_session
                .into_iter()
                .filter_map(|(id, mut group)| {
                    group.sort_by_key(|f| f.subagent); // the main file first: it names the session
                    session(id, &group, since, offset_secs)
                })
                .collect();
            sessions.sort_by(|a, b| b.last_active_secs.cmp(&a.last_active_secs).then_with(|| a.id.cmp(&b.id)));
            AccountUsage { provider: root.provider, label: root.label.clone(), sessions }
        })
        .collect()
}

fn session(id: &str, files: &[&FileUsage], since: Day, offset_secs: i32) -> Option<SessionUsage> {
    let mut cells: BTreeMap<(Day, &str), Tokens> = BTreeMap::new();
    let mut last_active_secs = 0;
    for rec in files.iter().flat_map(|f| &f.records) {
        let at = rec.bucket * BUCKET_SECS;
        let day = Day::from_epoch_secs(at, offset_secs);
        if day < since {
            continue;
        }
        *cells.entry((day, rec.model.as_str())).or_default() += rec.tokens;
        last_active_secs = last_active_secs.max(at);
    }
    if cells.is_empty() {
        return None;
    }
    let mut totals = Tokens::default();
    let mut by_model: BTreeMap<&str, u64> = BTreeMap::new();
    for ((_, model), tokens) in &cells {
        totals += *tokens;
        *by_model.entry(model).or_default() += tokens.total();
    }
    let model_main = by_model
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(model, _)| (*model).to_owned())
        .unwrap_or_default();
    let est_cost_usd = sum_cost(cells.iter().map(|((_, model), tokens)| cost_usd(model, tokens)));
    let days = cells
        .into_iter()
        .map(|((day, model), tokens)| DayTokens { day, model: model.to_owned(), tokens })
        .collect();
    let first = |pick: fn(&FileUsage) -> Option<&String>| files.iter().find_map(|f| pick(f)).cloned();
    let title = first(|f| f.title.as_ref()).or_else(|| first(|f| f.first_prompt.as_ref())).unwrap_or_else(|| UNTITLED.to_owned());
    let project = files.iter().find_map(|f| f.cwd.as_deref()).map(project_name).unwrap_or_default();
    Some(SessionUsage { id: id.to_owned(), title, project, model_main, days, totals, est_cost_usd, last_active_secs })
}
