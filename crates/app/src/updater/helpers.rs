use std::rc::Rc;

use super::{NoDriver, RequestSender, UpdateDriver, UpdateSender};

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
