use std::time::Duration;
use lathe_forge::ForgeError;
use super::*;
const S: fn(u64) -> Duration = Duration::from_secs;
/// Running checks are read often, settled ones less, and a merged or closed pull request not again.
#[test]
fn the_pace_follows_the_pull_request() {
    assert_eq!(next_delay(None, &Seen::Open { checks_running: true }), Some(S(10)));
    assert_eq!(next_delay(None, &Seen::Open { checks_running: false }), Some(S(30)));
    assert_eq!(next_delay(None, &Seen::Settled), None);
}
/// A failure backs off: 10 s, then double each time, to 5 min at most.
#[test]
fn failures_back_off_to_five_minutes() {
    let offline = Seen::Failed(ForgeError::Offline);
    let mut last = None;
    let mut waits = Vec::new();
    for _ in 0..7 {
        last = next_delay(last, &offline);
        waits.push(last.unwrap().as_secs());
    }
    assert_eq!(waits, [10, 20, 40, 80, 160, 300, 300]);
}
/// A rate limit waits as long as the forge says, and never less than 10 s.
#[test]
fn a_rate_limit_waits_as_the_forge_says() {
    assert_eq!(next_delay(None, &Seen::Failed(ForgeError::RateLimited { retry_after: Some(90) })), Some(S(90)));
    assert_eq!(next_delay(None, &Seen::Failed(ForgeError::RateLimited { retry_after: Some(2) })), Some(S(10)));
    assert_eq!(next_delay(Some(S(40)), &Seen::Failed(ForgeError::RateLimited { retry_after: None })), Some(S(80)));
}
/// What waiting does not mend stops the polling: the card says why, and a press reads again.
#[test]
fn what_waiting_does_not_mend_stops() {
    for error in [ForgeError::NotSignedIn, ForgeError::ToolMissing { tool: "gh".into() }, ForgeError::NotFound("#7".into()), ForgeError::Denied("no".into())] {
        assert_eq!(next_delay(None, &Seen::Failed(error.clone())), None, "{error}");
    }
}
