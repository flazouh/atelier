use crate::usage_history::structs::Day;

/// Epoch seconds of an ISO 8601 time such as `2026-09-25T19:40:14.448Z` or `...+02:00`.
pub(crate) fn iso_secs(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') || b[13] != b':' || b[16] != b':'
    {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<i64> {
        let digits = b.get(from..from + len)?;
        digits.iter().try_fold(0i64, |acc, d| d.is_ascii_digit().then(|| acc * 10 + i64::from(d - b'0')))
    };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = Day::new(year as i32, month as u8, day as u8).epoch_days();
    let mut secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    let mut rest = &b[19..];
    if rest.first() == Some(&b'.') {
        let digits = rest[1..].iter().take_while(|d| d.is_ascii_digit()).count();
        rest = &rest[1 + digits..];
    }
    match rest.first() {
        None | Some(b'Z') | Some(b'z') => Some(secs),
        Some(sign @ (b'+' | b'-')) => {
            let zone = std::str::from_utf8(&rest[1..]).ok()?;
            let (h, m) = zone.split_once(':').unwrap_or((zone.get(..2)?, zone.get(2..).unwrap_or("0")));
            let off = h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60;
            secs -= if *sign == b'+' { off } else { -off };
            Some(secs)
        }
        Some(_) => None,
    }
}
