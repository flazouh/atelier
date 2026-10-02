use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use atelier_voice::Press;
use gpui_kit::{AnyWindowHandle, WeakEntity};

use super::super::AgentSession;

/// How long the reason for a failed press stays in the box.
pub(super) const ERROR_SHOWN: Duration = Duration::from_secs(6);

/// The session a press belongs to, and its window.
pub(super) type Owner = (WeakEntity<AgentSession>, AnyWindowHandle);

/// Every press the engine has not finished with, and whose it is. A press stays here until its words, its failure or its
/// cancel come back, which can be long after the next press began.
pub(super) type Presses = Rc<RefCell<HashMap<Press, Owner>>>;
