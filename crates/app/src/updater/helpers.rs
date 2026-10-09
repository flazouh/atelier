use std::rc::Rc;

use super::{NoDriver, NoteLine, Remembered, RequestSender, UpdateDriver, UpdateSender, UpdateState};

/// The updater of this build. `requests` is where it asks the window for a restart, and `events` where it tells how an
/// update goes. The released Mac app carries Sparkle; every other build, and any platform with no updater, gets none and
/// drops both, which ends the window's wait.
#[cfg(target_os = "macos")]
pub fn driver(requests: RequestSender, events: UpdateSender) -> Rc<dyn UpdateDriver> {
    match super::SparkleDriver::start(requests, events) {
        Some(sparkle) => Rc::new(sparkle),
        None => Rc::new(NoDriver),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn driver(requests: RequestSender, events: UpdateSender) -> Rc<dyn UpdateDriver> {
    drop((requests, events));
    Rc::new(NoDriver)
}

/// `text` with its first letter in capitals: a note is written after its lead ("**Fixes:** a row fills the width") and shown on a line of its own.
fn capital(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| first.to_uppercase().chain(letters).collect())
}

/// The lines of a changelog written in markdown. A bullet that opens with a bold lead ("- **Fixes:** a row fills the
/// width") gives its lead and its text. Headings are the sheet's own, so they are dropped (older files still carry `###`
/// ones), and so is the `Released:` line. A changelog with no bullet is one line of its words, and none at all is none.
pub fn release_notes(markdown: &str) -> Vec<NoteLine> {
    let (mut bullets, mut words) = (Vec::new(), Vec::new());
    for line in markdown.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with(RELEASED)) {
        words.push(line);
        if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            bullets.push(match item.strip_prefix("**").and_then(|rest| rest.split_once("**")) {
                Some((lead, text)) => NoteLine {
                    lead: lead.trim().trim_end_matches(':').trim().to_string(),
                    text: capital(text.trim().trim_start_matches(':').trim()),
                },
                None => NoteLine { lead: item.trim().to_string(), text: String::new() },
            });
        }
    }
    if !bullets.is_empty() || words.is_empty() {
        return bullets;
    }
    vec![NoteLine { lead: "What changed".into(), text: words.join(" ") }]
}

/// The line of a changelog that dates it: `Released: 2026-10-09`, under the heading.
const RELEASED: &str = "Released:";

/// The date a changelog gives with its `Released:` line, as the sheet shows it ("Oct 9, 2026"). None when the line is
/// missing or is not a real date.
pub fn release_date(markdown: &str) -> Option<String> {
    let line = markdown.lines().map(str::trim).find_map(|line| line.strip_prefix(RELEASED))?;
    written_date(line.trim())
}

/// `2026-10-09` as "Oct 9, 2026". None when it is not a date of the calendar.
pub fn written_date(iso: &str) -> Option<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = iso.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let (year, month, day): (u32, usize, u32) = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    (1..=last).contains(&day).then(|| format!("{} {day}, {year}", MONTHS[month - 1]))
}

/// The numbers of a version such as `0.1.4`, or none when it is not that.
fn numbers(version: &str) -> Option<Vec<u32>> {
    version.trim().split('.').map(|part| part.parse().ok()).collect()
}

/// Whether `version` is an older release than `than`. False when either is not a version.
pub fn is_older(version: &str, than: &str) -> bool {
    matches!((numbers(version), numbers(than)), (Some(version), Some(than)) if version < than)
}

/// What to do at start with the changelog kept for `kept`, when `running` is the version that runs.
pub fn remembered(kept: &str, running: &str) -> Remembered {
    match (numbers(kept), numbers(running)) {
        (Some(kept), Some(running)) if kept == running => Remembered::Show,
        (Some(kept), Some(running)) if kept > running => Remembered::Wait,
        _ => Remembered::Forget,
    }
}

/// The changelog to keep when an update has just become ready, so the first start of that version can show it. None
/// for any other move of the state.
pub fn remember_on_ready(before: &UpdateState, after: &UpdateState) -> Option<atelier_settings::WhatsNew> {
    match (before, after) {
        (UpdateState::Ready { .. }, _) => None,
        (_, UpdateState::Ready { version, notes }) if !version.is_empty() => {
            Some(atelier_settings::WhatsNew { version: version.clone(), notes: notes.clone() })
        }
        _ => None,
    }
}

#[cfg(all(target_os = "macos", not(test)))]
unsafe extern "C" {
    fn atelier_bundle_version() -> *const std::ffi::c_char;
}

/// The version that runs: the bundle's on the Mac, which is the one the update feed is compared with, else the one this
/// build was made with.
pub fn running_version() -> String {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        // SAFETY: the native side returns NULL or a NUL-terminated string that lives as long as the bundle's dictionary.
        let bundled = unsafe { atelier_bundle_version() };
        if !bundled.is_null() {
            return unsafe { std::ffi::CStr::from_ptr(bundled) }.to_string_lossy().into_owned();
        }
    }
    env!("CARGO_PKG_VERSION").to_string()
}
