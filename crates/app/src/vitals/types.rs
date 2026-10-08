use std::rc::Rc;
use gpui_kit::{App, Window};
/// What a press on a part of the bar does. The shell hands it in; the bar only calls it.
pub type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
