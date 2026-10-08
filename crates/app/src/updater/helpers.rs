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

/// Which kind a `###` heading names, by a word in it: "New" or "Added", "Faster" or "Performance", "Fixed" or "Bug", "Changed",
/// "Design"; else "Improved".
fn kind_of(heading: &str) -> atelier_ui::ReleaseKind {
    use atelier_ui::ReleaseKind::{Added, Changed, Design, Faster, Fixed, Improved};
    let heading = heading.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| heading.contains(word));
    if has(&["new", "add"]) {
        Added
    } else if has(&["fix", "bug"]) {
        Fixed
    } else if has(&["fast", "perf", "speed"]) {
        Faster
    } else if has(&["chang"]) {
        Changed
    } else if has(&["design", "look"]) {
        Design
    } else {
        Improved
    }
}
/// The colour of a kind of note, from the app's palette: green for added, purple for improved, orange for faster, red for
/// fixed, blue for changed and pink for design, each as dark as the page needs for it to read.
pub fn kind_color(kind: atelier_ui::ReleaseKind, theme: &atelier_ui::theme::Theme) -> gpui_kit::Hsla {
    use crate::palette::Hue;
    use atelier_ui::ReleaseKind::{Added, Changed, Design, Faster, Fixed, Improved};
    match kind {
        Added => Hue::Green,
        Improved => Hue::Purple,
        Faster => Hue::Orange,
        Fixed => Hue::Red,
        Changed => Hue::Blue,
        Design => Hue::Pink,
    }
    .on(theme.popover)
}
/// The lines of a changelog written in markdown. A bullet that opens with a bold lead ("- **Fixes:** a row fills the
/// width") gives its lead and its text, and takes its kind from the last `###` heading ("### New", "### Improved",
/// "### Faster", "### Fixed", "### Changed", "### Design"; Improved before any). Other headings are the sheet's own, so they are dropped. A changelog with no bullet
/// is one line of its words, and none at all is none.
pub fn release_notes(markdown: &str) -> Vec<NoteLine> {
    let mut kind = atelier_ui::ReleaseKind::Improved;
    let (mut bullets, mut words) = (Vec::new(), Vec::new());
    for line in markdown.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(heading) = line.strip_prefix("###") {
            kind = kind_of(heading);
        } else if !line.starts_with('#') {
            words.push(line);
            if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                bullets.push(match item.strip_prefix("**").and_then(|rest| rest.split_once("**")) {
                    Some((lead, text)) => NoteLine {
                        kind,
                        lead: lead.trim().trim_end_matches(':').trim().to_string(),
                        text: capital(text.trim().trim_start_matches(':').trim()),
                    },
                    None => NoteLine { kind, lead: item.trim().to_string(), text: String::new() },
                });
            }
        }
    }
    if !bullets.is_empty() || words.is_empty() {
        return bullets;
    }
    vec![NoteLine { kind: atelier_ui::ReleaseKind::Improved, lead: "What changed".into(), text: words.join(" ") }]
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
