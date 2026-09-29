//! Times from the forge, as seconds since the Unix epoch. Forges send RFC 3339 in UTC.

/// `2026-09-29T15:14:16Z` as epoch seconds. Anything else is `None`.
pub(crate) fn parse(text: &str) -> Option<u64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut d = date.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':').map(|part| part.parse::<i64>().ok());
    let (hour, minute, second) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // Days from civil, after Howard Hinnant.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

/// "2h ago", "3d ago": how long before `now`, in one unit.
pub(crate) fn ago(now: u64, then: u64) -> String {
    let seconds = now.saturating_sub(then);
    let (n, unit) = match seconds {
        0..=59 => return "just now".into(),
        60..=3599 => (seconds / 60, "m"),
        3600..=86_399 => (seconds / 3600, "h"),
        86_400..=2_591_999 => (seconds / 86_400, "d"),
        2_592_000..=31_535_999 => (seconds / 2_592_000, "mo"),
        _ => (seconds / 31_536_000, "y"),
    };
    format!("{n}{unit} ago")
}

#[cfg(test)]
mod tests;
