//! A reference from a tool card opens in its own screen: a task in Tasks, a channel in Messages, a mailbox or a thread in Mail.
use atelier_capabilities::Ref;
use gpui_kit::{Context, Entity, Window};

use super::{structs::Shell, view::ShellView};
use crate::open_project::OpenProject;

impl Shell {
    /// Shows `reference` where it lives. A reference of a kind no screen shows changes nothing.
    pub(super) fn open_ref(
        &mut self,
        project: &Entity<OpenProject>,
        reference: &Ref,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match reference.capability.as_str() {
            "tasks" => {
                if let Some(at) = self.projects.iter().position(|p| p == project) {
                    self.active = at;
                }
                // `show_tasks` closes the view when it is open already, so it is only asked when the view is not in front.
                if self.view != ShellView::Tasks {
                    self.show_tasks(window, cx);
                }
                let key = reference.id.clone();
                project.update(cx, |p, cx| {
                    if let Some(slot) = &p.tasks {
                        slot.pane
                            .update(cx, |pane, cx| pane.show_key(&key, window, cx));
                    }
                });
            }
            "messaging" => {
                self.show_messages(window, cx);
                if let Some(pane) = self.messages.clone() {
                    pane.update(cx, |pane, cx| pane.open_ref(reference, cx));
                }
            }
            "mail" => {
                self.show_mail(window, cx);
                if let Some(pane) = self.mail.clone() {
                    pane.update(cx, |pane, cx| pane.open_ref(reference, window, cx));
                }
            }
            _ => {}
        }
    }
}
