use std::rc::Rc;

use super::{NoDriver, NoteLine, RequestSender, UpdateDriver, UpdateSender};

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
