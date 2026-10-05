use std::rc::Rc;

use super::super::UpdateDriver;

/// The app's handle on the platform updater.
pub struct Updater {
    pub(in super::super) driver: Rc<dyn UpdateDriver>,
}
