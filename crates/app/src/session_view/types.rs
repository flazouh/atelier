use std::rc::Rc;

use gpui_kit::{App, Window};

/// What a choice in the panel's menu does.
pub(super) type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;
