use atelier_capabilities::{
    CapError, Ref,
    mail::{Draft, MailOperation, reply_recipients},
};
use gpui_kit::{AppContext, Context, SharedString, Window};

use super::super::{
    helpers::write_draft,
    structs::{Compose, Facts, Held, MailPane, Outcome},
    types::{Load, Press, Problem},
};
use atelier_capabilities::CapResult;

impl MailPane {
    /// The reply box is there when the thread is read, the provider lists `create_draft`, and the reader may act. A provider
    /// that cannot write has no box at all.
    pub(in crate::mail::pane) fn compose_shown(&self) -> bool {
        self.open.is_some()
            && matches!(self.thread_load, Load::Ready)
            && self.thread.as_ref().is_some_and(|t| !t.messages.is_empty())
            && self.can(MailOperation::CreateDraft)
            && self.effective_problem() != Some(Problem::SignedOut)
    }

    /// The words in the reply box.
    fn words(&self, cx: &gpui_kit::App) -> SharedString {
        self.compose.read(cx).value()
    }

    /// What the reply box says of itself: who the reply goes to, where the draft stands, and whether the words can be written.
    pub(in crate::mail::pane) fn compose_facts(&self, cx: &gpui_kit::App) -> Option<Facts> {
        if !self.compose_shown() {
            return None;
        }
        let last = self.thread.as_ref()?.messages.last()?;
        let account = self.account()?;
        let (to, _) = reply_recipients(last, &account.choice.account, false);
        let to: SharedString = match to.is_empty() {
            true => "To nobody: the message has no one to answer".into(),
            false => format!(
                "To {}",
                to.iter()
                    .map(|c| c.address.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into(),
        };
        let text = self.words(cx);
        let status = match (&self.draft, text.trim().is_empty()) {
            (None, true) => None,
            (None, false) => Some("Not saved".into()),
            (Some(d), _) if d.text == text.as_ref() => Some("Draft saved".into()),
            (Some(_), _) => Some("Changes not saved".into()),
        };
        Some(Facts {
            to,
            status,
            empty: text.trim().is_empty(),
            // A saved draft can be changed only where the provider lists `update_draft`; a new one can always be written.
            editable: self.draft.is_none() || self.can(MailOperation::UpdateDraft),
        })
    }

    /// What the reply box is made of for the drawing code.
    pub(in crate::mail::pane) fn compose_view(&self, cx: &mut Context<Self>) -> Option<Compose> {
        let Facts {
            to,
            status,
            empty,
            editable,
        } = self.compose_facts(cx)?;
        let pane = cx.entity().downgrade();
        let press = |send: bool| -> Press {
            let pane = pane.clone();
            std::rc::Rc::new(move |window, cx| {
                drop(pane.update(cx, |p, cx| p.write_reply(send, window, cx)))
            })
        };
        Some(Compose {
            to,
            status,
            input: self.compose.clone(),
            focus: self.compose_focus.clone(),
            editable,
            busy: self.working,
            empty,
            save: Some(press(false)),
            send: self.can(MailOperation::Send).then(|| press(true)),
        })
    }

    /// Saves the words of the box as the draft of the open thread (made, or changed from the version held), and with `send` sends
    /// that very version. The box shows what the draft says when the call returns, and a change of the draft behind the pane's
    /// back is a conflict that sends nothing.
    pub fn write_reply(&mut self, send: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(account), Some(thread), Some(open)) =
            (self.account(), self.thread.clone(), self.open.clone())
        else {
            return;
        };
        let Some(last) = thread.messages.last().cloned() else {
            return;
        };
        if self.working
            || !account.caps.can(MailOperation::CreateDraft)
            || (send && !account.caps.can(MailOperation::Send))
        {
            return;
        }
        let text = self.words(cx).to_string();
        if text.trim().is_empty() {
            return;
        }
        let (provider, me, held, epoch) = (
            account.provider.clone(),
            self.me.clone(),
            self.draft.clone(),
            self.epoch,
        );
        self.working = true;
        (self.said, self.said_reading) = (None, false);
        let had_draft = held.is_some();
        let writing = cx.background_spawn({
            let text = text.clone();
            async move { write_draft(provider.as_ref(), &me, held.as_ref(), &last, &text, send) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = writing.await;
            this.update_in(cx, |this, window, cx| {
                this.written(result, open, text, had_draft, epoch, window, cx)
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn written(
        &mut self,
        result: CapResult<Outcome>,
        thread: Ref,
        text: String,
        had_draft: bool,
        epoch: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if epoch != self.epoch {
            return;
        }
        self.working = false;
        let here = self.open.as_ref() == Some(&thread);
        match result {
            Ok(Outcome { draft, sent: None }) => self.keep_draft(&thread, here, Some(draft), text),
            Ok(Outcome {
                draft: _,
                sent: Some(Ok(_)),
            }) => {
                // The draft is gone with the message it became: the box is empty, and the thread has one more message.
                self.held.retain(|h| h.thread != thread);
                if here {
                    self.draft = None;
                    self.compose.update(cx, |c, cx| c.set_value("", window, cx));
                    self.reload(cx);
                }
            }
            Ok(Outcome {
                draft,
                sent: Some(Err(CapError::Conflict { current })),
            }) => {
                // The draft the provider holds is not the one that was saved a moment ago. Nothing was sent: the box shows the
                // draft as it is now, and the reader reads it again before they press Send.
                let now: Draft = serde_json::from_value(current).unwrap_or(draft);
                if here {
                    let shown = now.text.clone();
                    self.compose
                        .update(cx, |c, cx| c.set_value(shown, window, cx));
                }
                self.keep_draft(&thread, here, Some(now.clone()), now.text);
                self.said = Some(
                    "The draft changed before it was sent. Read it again, then press Send.".into(),
                );
            }
            Ok(Outcome {
                draft,
                sent: Some(Err(error)),
            }) => {
                // The draft is saved; only the send failed. The reader's words stay, as saved.
                self.keep_draft(&thread, here, Some(draft), text);
                self.fail(error, MailOperation::Send, "Could not send", cx);
            }
            Err(CapError::Conflict { current }) if had_draft => {
                // Someone changed the draft since the pane read it. The reader's words stay in the box; the version now held is
                // the one their next save changes.
                if let Ok(now) = serde_json::from_value::<Draft>(current) {
                    self.keep_draft(&thread, here, Some(now), text);
                }
                self.said = Some(
                    "The draft was changed elsewhere. Save again to replace it with your words."
                        .into(),
                );
            }
            Err(error) => {
                let operation = match had_draft {
                    true => MailOperation::UpdateDraft,
                    false => MailOperation::CreateDraft,
                };
                self.fail(error, operation, "Could not save the draft", cx);
            }
        }
        cx.notify();
    }

    /// Remembers `draft` for `thread`: in the pane when it is the open thread, else with what the pane keeps for the others.
    fn keep_draft(&mut self, thread: &Ref, here: bool, draft: Option<Draft>, text: String) {
        if here {
            self.draft = draft;
            return;
        }
        self.held.retain(|h| &h.thread != thread);
        self.held.push(Held {
            thread: thread.clone(),
            draft,
            text: text.into(),
        });
    }
}
