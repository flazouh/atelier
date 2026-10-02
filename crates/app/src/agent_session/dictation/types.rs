use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui_kit::{AnyWindowHandle, WeakEntity};

use super::super::AgentSession;

/// How long the reason for a failed press stays in the box.
pub(super) const ERROR_SHOWN: Duration = Duration::from_secs(6);

/// The session whose press the engine is serving, and its window; none between presses.
pub(super) type Owner = Rc<RefCell<Option<(WeakEntity<AgentSession>, AnyWindowHandle)>>>;
