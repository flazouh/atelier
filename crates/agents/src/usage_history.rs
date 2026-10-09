//! What the coding agents spent, read from their own session logs on this machine. [`read`] walks the logs of each
//! account (a Claude config folder such as `~/.claude`, or `~/.codex`) and returns a [`UsageHistory`]: per account,
//! the sessions with their tokens by day and model, and an estimated cost. Query helpers on [`UsageHistory`] give
//! totals per day, per model and the top sessions.
//!
//! Formats read (checked against real files):
//! - Claude Code: `<config>/projects/<project>/<session>.jsonl`, plus `<session>/subagents/*.jsonl`, which count
//!   for the parent session. `assistant` lines carry `message.usage`; one API message can repeat on several lines
//!   (streaming), so lines with both `message.id` and `requestId` are deduped and the last one wins. The title is the
//!   last `custom-title`, else `ai-title`, else the first user prompt.
//! - Codex: `<codex>/sessions/YYYY/MM/DD/rollout-*.jsonl`. `token_count` events carry running totals; each event
//!   counts the delta of the totals, so a repeated event adds nothing. Codex's `input_tokens` holds the cached and
//!   cache-written tokens too, so they are taken out of `input`.
//!
//! Costs are estimates from a public price table (`consts.rs`); an unknown model has no cost.
//!
//! Out of scope: logs of a remote host (a remote project's agent runs on another machine). Known limit: a forked
//! Claude session copies earlier lines into a new file, and those lines count in both files.
//!
//! Speed: only files modified since the start day (minus one day) are opened, files parse in parallel, and a
//! [`Cache`] the caller keeps skips files whose path, mtime and size did not change. Timestamps are kept in 15
//! minute buckets, so the day split is exact for every real UTC offset (they are multiples of 15 minutes).
//! Nothing here panics: bad lines, unreadable files and folders are skipped and counted in
//! [`UsageHistory::skipped`].

mod consts;
mod helpers;
mod impls;
mod structs;
mod types;

pub use helpers::{estimate_cost_usd, read, read_cached};
pub use structs::{
    AccountDays, AccountRoot, AccountUsage, Cache, Day, DayTokens, DayTotal, ModelTotal, RangedSession, Roots,
    SessionUsage, Tokens, UsageHistory,
};
pub use types::Provider;

#[cfg(test)]
mod tests;
