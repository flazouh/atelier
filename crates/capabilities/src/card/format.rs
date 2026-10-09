//! Reading values out of a tool result, and printing them. Nothing here evaluates anything but a path, a hole, and a format.
use super::types::Format;

/// The value at a path such as `$.issues[0].title`. A path with anything but dots and indexes finds nothing.
pub(super) fn eval<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut rest = path.strip_prefix('$')?;
    let mut at = root;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            let (name, tail) = after.split_at(end);
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            at = at.get(name)?;
            rest = tail;
        } else {
            let after = rest.strip_prefix('[')?;
            let (index, tail) = after.split_once(']')?;
            at = at.get(index.parse::<usize>().ok()?)?;
            rest = tail;
        }
    }
    Some(at)
}

/// A JSON value as text. Null is empty; a whole number has no `.0`.
pub(super) fn text_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

/// Whether two JSON values are equal, numbers by value.
pub(super) fn same(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// `{$.a}` and `{$.a|format}` holes, replaced from `root`. A hole that finds nothing is empty. A brace that opens no hole stays.
pub(super) fn fill(template: &str, root: &serde_json::Value, now: i64) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{$") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let hole = &after[..close];
                let (path, format) = match hole.split_once('|') {
                    Some((path, name)) => (path, Format::from_name(name)),
                    None => (hole, None),
                };
                if let Some(value) = eval(root, path) {
                    out.push_str(&show(value, format, now));
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// A value as text, in a format when it has one and the value is a number.
pub(super) fn show(value: &serde_json::Value, format: Option<Format>, now: i64) -> String {
    match (format, value.as_f64()) {
        (Some(format), Some(n)) => print(n, format, now),
        _ => text_of(value),
    }
}

fn print(n: f64, format: Format, now: i64) -> String {
    match format {
        Format::Count => count(n),
        Format::RelativeTime => relative(n as i64, now),
        Format::AbsoluteTime => absolute(n as i64),
        Format::DurationMs => duration(n),
        Format::Bytes => bytes(n),
        Format::Percent => format!("{}%", n.round() as i64),
    }
}

fn one_decimal(n: f64) -> String {
    let rounded = (n * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

fn count(n: f64) -> String {
    let a = n.abs();
    match a {
        a if a < 1_000.0 => format!("{}", n.round() as i64),
        a if a < 1_000_000.0 => format!("{}k", one_decimal(n / 1_000.0)),
        a if a < 1_000_000_000.0 => format!("{}M", one_decimal(n / 1_000_000.0)),
        _ => format!("{}B", one_decimal(n / 1_000_000_000.0)),
    }
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

fn relative(then: i64, now: i64) -> String {
    let diff = now - then;
    if diff < 45_000 {
        return "just now".into();
    }
    let minutes = diff / 60_000;
    if minutes < 60 {
        return format!("{} min ago", minutes.max(1));
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{} ago", plural(hours, "hour"));
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{} ago", plural(days, "day"));
    }
    absolute(then)
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// `2025-10-09 12:00 UTC`, from milliseconds since the epoch.
fn absolute(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let in_day = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        in_day / 3600,
        in_day % 3600 / 60
    )
}

/// The calendar date of a day number (days since 1970-01-01).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn duration(ms: f64) -> String {
    match ms {
        ms if ms < 1_000.0 => format!("{} ms", ms.round() as i64),
        ms if ms < 60_000.0 => format!("{} s", one_decimal(ms / 1_000.0)),
        ms if ms < 3_600_000.0 => format!("{} min", one_decimal(ms / 60_000.0)),
        ms => format!("{} h", one_decimal(ms / 3_600_000.0)),
    }
}

fn bytes(n: f64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = n;
    let mut unit = 0;
    while size.abs() >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{} {}", one_decimal(size), UNITS[unit])
}
