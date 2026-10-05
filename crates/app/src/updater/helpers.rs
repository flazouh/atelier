use std::rc::Rc;

use super::{NoDriver, RequestSender, UpdateDriver};

/// The updater of this build. `requests` is where it asks the window for a restart. The released Mac app carries
/// Sparkle; every other build, and any platform with no updater, gets none and drops `requests`, which ends the
/// window's wait.
#[cfg(target_os = "macos")]
pub fn driver(requests: RequestSender) -> Rc<dyn UpdateDriver> {
    match super::SparkleDriver::start(requests) {
        Some(sparkle) => Rc::new(sparkle),
        None => Rc::new(NoDriver),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn driver(requests: RequestSender) -> Rc<dyn UpdateDriver> {
    drop(requests);
    Rc::new(NoDriver)
}
