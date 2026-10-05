use super::super::{NoDriver, UpdateDriver};

impl UpdateDriver for NoDriver {
    fn available(&self) -> bool {
        false
    }

    fn check(&self) {}
}
