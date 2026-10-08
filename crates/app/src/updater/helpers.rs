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

/// The lines of a changelog written in markdown. A bullet that opens with a bold lead ("- **Fixes:** a row fills the
/// width") gives its lead and its text. Headings are the sheet's own, so they are dropped. A changelog with no bullet
/// is one line of its words, and none at all is none.
pub fn release_notes(markdown: &str) -> Vec<NoteLine> {
    let lines: Vec<&str> = markdown.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let bullets: Vec<NoteLine> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")))
        .map(|item| match item.strip_prefix("**").and_then(|rest| rest.split_once("**")) {
            Some((lead, text)) => NoteLine {
                lead: lead.trim().trim_end_matches(':').trim().to_string(),
                text: text.trim().trim_start_matches(':').trim().to_string(),
            },
            None => NoteLine { lead: item.trim().to_string(), text: String::new() },
        })
        .collect();
    if !bullets.is_empty() || lines.is_empty() {
        return bullets;
    }
    vec![NoteLine { lead: "What changed".into(), text: lines.join(" ") }]
}

/// The numbers of a version such as `0.1.4`, or none when it is not that.
fn numbers(version: &str) -> Option<Vec<u32>> {
    version.trim().split('.').map(|part| part.parse().ok()).collect()
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
