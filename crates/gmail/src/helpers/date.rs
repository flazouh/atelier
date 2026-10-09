/// Reads a date as Gmail shows it, such as `Fri, 18 Sept 2026, 10:39`, `Fri, Sep 18, 2026, 10:39 AM` or
/// `ven. 18 sept. 2026, 10:39`, as milliseconds since the epoch. The time is read as UTC, because the text carries no offset.
/// A text with no day, month or year gives `None`.
pub(crate) fn parse_display_date(text: &str) -> Option<i64> {
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    let is_number = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit());
    let year = words
        .iter()
        .find(|w| is_number(w) && w.len() == 4)
        .and_then(|w| w.parse::<i64>().ok())
        .filter(|y| (1970..=2200).contains(y))?;
    let day_at = words.iter().position(|w| {
        is_number(w) && w.len() <= 2 && w.parse::<u32>().is_ok_and(|d| (1..=31).contains(&d))
    })?;
    let day = words[day_at].parse::<i64>().ok()?;
    // The month is next to the day: after it (`18 Sept`) or before it (`Sep 18`). A weekday such as the French `mar.`
    // is never next to the day on the other side of the month.
    let month = [day_at + 1, day_at.wrapping_sub(1)]
        .into_iter()
        .filter_map(|at| words.get(at))
        .find_map(|w| month_of(w))?;
    let (mut hour, minute) = words
        .iter()
        .find(|w| w.contains(':'))
        .and_then(|w| {
            let mut parts = w.split(':');
            let hour = parts.next()?.parse::<i64>().ok()?;
            let minute = parts.next()?.parse::<i64>().ok()?;
            ((0..24).contains(&hour) && (0..60).contains(&minute)).then_some((hour, minute))
        })
        .unwrap_or((0, 0));
    let meridiem = words
        .iter()
        .find_map(|w| match w.to_lowercase().trim_matches('.') {
            "am" => Some(false),
            "pm" => Some(true),
            _ => None,
        });
    match meridiem {
        Some(true) if hour < 12 => hour += 12,
        Some(false) if hour == 12 => hour = 0,
        _ => {}
    }
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 60 + minute) * 60_000)
}

/// The month a word names, in English or French, by its first three letters. It must be at least three letters long, and
/// letters only apart from a dot.
fn month_of(word: &str) -> Option<i64> {
    let word = word.trim_end_matches('.').to_lowercase();
    if word.chars().count() < 3 || !word.chars().all(char::is_alphabetic) {
        return None;
    }
    let head: String = word.chars().take(3).collect();
    Some(match head.as_str() {
        "jan" => 1,
        "feb" | "fév" | "fev" => 2,
        "mar" => 3,
        "apr" | "avr" => 4,
        "may" | "mai" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" | "aoû" | "aou" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" | "déc" => 12,
        _ => return None,
    })
}

/// Days from 1970-01-01 to the civil date (proleptic Gregorian). After Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
