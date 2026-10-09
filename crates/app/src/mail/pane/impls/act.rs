use atelier_capabilities::{
    Actor, CapResult, Ref,
    mail::{MailOperation, MailProvider},
};
use gpui_kit::{AppContext, Context};

use super::super::{structs::MailPane, types::Menu};

impl MailPane {
    /// Runs `call` on the open thread off the UI thread, as `operation`. `leaves` is for a change after which the thread is no
    /// longer in the open mailbox (archive, trash, move), so the reading pane closes. The pane reads again after a good answer,
    /// for what the provider now says, and never changes its own copy first.
    fn change(
        &mut self,
        operation: MailOperation,
        doing: &'static str,
        leaves: bool,
        cx: &mut Context<Self>,
        call: impl FnOnce(&dyn MailProvider, &Ref, &Actor) -> CapResult<()> + Send + 'static,
    ) {
        let (Some(account), Some(thread)) = (self.account(), self.open.clone()) else {
            return;
        };
        if self.working || !account.caps.can(operation) {
            return;
        }
        let (provider, me, epoch) = (account.provider.clone(), self.me.clone(), self.epoch);
        self.working = true;
        self.menu = None;
        (self.said, self.said_reading) = (None, false);
        let doing_it = cx.background_spawn(async move { call(provider.as_ref(), &thread, &me) });
        cx.spawn(async move |this, cx| {
            let result = doing_it.await;
            this.update(cx, |this, cx| this.changed(result, operation, doing, leaves, epoch, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn changed(
        &mut self,
        result: CapResult<()>,
        operation: MailOperation,
        doing: &str,
        leaves: bool,
        epoch: u64,
        cx: &mut Context<Self>,
    ) {
        if epoch != self.epoch {
            return;
        }
        self.working = false;
        match result {
            Ok(()) => {
                if leaves {
                    self.close_thread(cx);
                }
                self.reload(cx);
            }
            Err(error) => self.fail(error, operation, doing, cx),
        }
        cx.notify();
    }

    /// Marks the open thread read, or unread. Only the reader's press does this.
    pub fn mark_read(&mut self, read: bool, cx: &mut Context<Self>) {
        self.change(MailOperation::MarkRead, "Could not mark the thread", false, cx, move |p, thread, me| {
            p.mark_read(thread, read, me)
        });
    }

    pub fn star(&mut self, starred: bool, cx: &mut Context<Self>) {
        self.change(MailOperation::Star, "Could not star the thread", false, cx, move |p, thread, me| {
            p.star(thread, starred, me)
        });
    }

    pub fn archive(&mut self, cx: &mut Context<Self>) {
        self.change(MailOperation::Archive, "Could not archive the thread", true, cx, |p, thread, me| {
            p.archive(thread, me)
        });
    }

    pub fn trash(&mut self, cx: &mut Context<Self>) {
        self.change(MailOperation::Trash, "Could not move the thread to the trash", true, cx, |p, thread, me| {
            p.trash(thread, me)
        });
    }

    /// Moves the open thread to `mailbox`.
    pub fn move_to(&mut self, mailbox: &Ref, cx: &mut Context<Self>) {
        let mailbox = mailbox.clone();
        self.change(MailOperation::Move, "Could not move the thread", true, cx, move |p, thread, me| {
            p.move_to(thread, &mailbox, me)
        });
    }

    /// Adds `label` to the open thread, or takes it off.
    pub fn label(&mut self, label: &Ref, add: bool, cx: &mut Context<Self>) {
        let label = vec![label.clone()];
        self.change(MailOperation::Label, "Could not change the labels", false, cx, move |p, thread, me| {
            match add {
                true => p.label(thread, &label, &[], me),
                false => p.label(thread, &[], &label, me),
            }
        });
    }

    /// Opens or closes a small menu of the reading pane's head.
    pub fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = (self.menu != Some(menu)).then_some(menu);
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    /// The reader pressed an address of a body. Nothing opens yet: the address is shown and waits for a yes.
    pub fn ask_link(&mut self, address: &str, cx: &mut Context<Self>) {
        self.link = Some(address.to_string().into());
        cx.notify();
    }

    pub fn cancel_link(&mut self, cx: &mut Context<Self>) {
        self.link = None;
        cx.notify();
    }

    /// Opens the address that waits, in the system browser, when it is a web address. Nothing else opens.
    pub fn open_link(&mut self, cx: &mut Context<Self>) {
        if let Some(address) = self.link.take()
            && is_web(&address)
        {
            cx.open_url(&address);
        }
        cx.notify();
    }

    /// The reader asked to see the whole of a long message.
    pub fn show_all(&mut self, message: &Ref, cx: &mut Context<Self>) {
        if !self.whole.contains(message) {
            self.whole.push(message.clone());
        }
        cx.notify();
    }
}

/// Whether `address` is a web address: the only kind the screen opens.
fn is_web(address: &str) -> bool {
    let lower = address.to_ascii_lowercase();
    ["https://", "http://"].iter().any(|scheme| lower.starts_with(scheme))
}
