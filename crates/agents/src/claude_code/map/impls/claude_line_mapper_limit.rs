//! The account's usage limit.

use crate::claude_code::wire::{RateLimit, RateLimitInfo};
use crate::session::{Event, Limit, LimitState, LimitWindow};

use super::super::structs::ClaudeLineMapper;

impl ClaudeLineMapper {
    pub(super) fn limit_told(&mut self, event: RateLimit) -> Vec<Event> {
        let Some(told) = event.rate_limit_info.and_then(limit) else { return Vec::new() };
        if self.limit.replace(told) == Some(told) { Vec::new() } else { vec![Event::Limit(told)] }
    }
}

/// What `claude` tells of the account's usage limit, or `None` for a status it does not know.
pub(super) fn limit(info: RateLimitInfo) -> Option<Limit> {
    let state = match info.status.as_str() {
        "allowed" => LimitState::Clear,
        "allowed_warning" => LimitState::Near,
        "rejected" => LimitState::Reached,
        _ => return None,
    };
    let window = info.rate_limit_type.as_deref().and_then(|kind| match kind {
        "five_hour" => Some(LimitWindow::FiveHour),
        "overage" => Some(LimitWindow::Overage),
        weekly if weekly.starts_with("seven_day") => Some(LimitWindow::Weekly),
        _ => None,
    });
    Some(Limit { state, resets_at: info.resets_at, window })
}
