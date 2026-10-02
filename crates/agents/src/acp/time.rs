//! The time an agent gives a session's last activity, as seconds since the Unix epoch. ACP sends an
//! ISO 8601 date and time; an agent that sends a bare number of seconds or milliseconds reads too.

/// `2026-10-01T18:51:00Z`, `2026-10-01T18:51:00.123+02:00`, `1790880660` or `1790880660123`.
pub(super) fn epoch_seconds(text: &str) -> Option<u64> {
    let text = text.trim();
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        let number: u64 = text.parse().ok()?;
        // Past the year 33658 in seconds, so it counts milliseconds.
        return Some(if number >= 1_000_000_000_000 { number / 1000 } else { number });
    }
    let (date, rest) = text.split_once(['T', 't', ' '])?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let zone_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(zone_at);
    let mut clock = clock.split(':');
    let hour: i64 = clock.next()?.parse().ok()?;
    let minute: i64 = clock.next()?.parse().ok()?;
    let second = clock.next().map_or(Some(0), |s| {
        let (whole, fraction) = s.split_once('.').unwrap_or((s, "0"));
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        (digits(whole) && digits(fraction)).then(|| whole.parse::<i64>().ok()).flatten()
    })?;
    let offset = match zone {
        "" | "Z" | "z" => 0,
        _ => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            // ISO 8601 writes an offset as `±hh`, `±hhmm` or `±hh:mm`, two digits for each part.
            let (hours, minutes) = match zone[1..].split_once(':') {
                Some((hours, minutes)) => (hours, minutes),
                None if zone.len() == 3 => (&zone[1..], "00"),
                None if zone.len() == 5 => zone[1..].split_at(2),
                None => return None,
            };
            let two_digits = |part: &str| (part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<i64>().ok()).flatten();
            let (hours, minutes) = (two_digits(hours)?, two_digits(minutes)?);
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 3600 + minutes * 60)
        }
    };
    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(seconds).ok()
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
