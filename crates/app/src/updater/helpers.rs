use std::rc::Rc;

use super::{NoDriver, RequestSender, UpdateDriver};

/// The updater of this build. `requests` is where it asks the window for a restart; a build with no platform
/// updater drops it, which ends the window's wait.
pub fn driver(requests: RequestSender) -> Rc<dyn UpdateDriver> {
    drop(requests);
    Rc::new(NoDriver)
}
