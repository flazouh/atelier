use std::sync::Arc;

use gpui_kit::{App, AppContext, Entity, Task, WeakEntity};
use atelier_forge::{Forge, PullRef};

use super::structs::{PullWatch, Watches};

/// The watch of `reference`: the one already there, or a new one that reads at once.
pub fn watch(reference: PullRef, forge: Arc<dyn Forge>, cx: &mut App) -> Entity<PullWatch> {
    let known = cx.default_global::<Watches>().0.get(&reference).and_then(WeakEntity::upgrade);
    if let Some(watch) = known {
        return watch;
    }
    let made = cx.new(|cx| {
        let mut watch = PullWatch {
            reference: reference.clone(),
            forge,
            pull: None,
            checks: Vec::new(),
            unread: None,
            failure: None,
            next: None,
            drawn: 0,
            drawn_at_read: 0,
            active: true,
            paused: false,
            polling: Task::ready(()),
        };
        watch.read_now(cx);
        watch
    });
    cx.default_global::<Watches>().0.insert(reference, made.downgrade());
    made
}
