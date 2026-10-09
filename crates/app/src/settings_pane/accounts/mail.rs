//! The Mail group of the Accounts section: Gmail through the reader's own `gmailcli`, on this machine or over SSH on the
//! host that holds the browser login. Like the Chat cards it keeps only facts: the address, the host and the path.
use atelier_settings::GmailSaved;
use atelier_ui::{Segment, Segmented, icon::IconName, scale::px, theme::Theme};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Task, Window,
    component::input::InputState, div, prelude::FluentBuilder,
};

use super::{
    AccountsPage, Checked, badge_of, check_line, row_note,
    parts::{Buttons, actions, labelled, note, typed},
};
use crate::{
    accounts::{self, Kind, Row, Rows},
    settings_pane::{
        SettingsPane,
        providers::{card, card_head_of, icon_tile},
    },
};

const GMAIL: Buttons = Buttons { test: "gmail-test", save: "gmail-save", forget: "gmail-forget" };

/// What the Mail card holds while the page is open.
#[derive(Default)]
pub(crate) struct MailPage {
    pub(crate) check: Checked,
    /// `gmailcli` runs over SSH, rather than here.
    pub(crate) over_ssh: bool,
    fields: Option<MailFields>,
    _task: Option<Task<()>>,
}

struct MailFields {
    address: Entity<InputState>,
    host: Entity<InputState>,
    program: Entity<InputState>,
}

fn field(placeholder: &str, value: &str, window: &mut Window, cx: &mut Context<SettingsPane>) -> Entity<InputState> {
    let (placeholder, value) = (placeholder.to_string(), value.to_string());
    cx.new(|cx| {
        let mut field = InputState::new(window, cx).placeholder(placeholder);
        field.set_value(value, window, cx);
        field
    })
}

impl SettingsPane {
    /// Makes the fields, filled with what is saved, the first time the section is drawn.
    pub(super) fn ensure_mail_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.accounts.mail.fields.is_some() {
            return;
        }
        let gmail = self.accounts.saved.gmail.clone().unwrap_or_default();
        self.accounts.mail.over_ssh = !gmail.host.is_empty();
        self.accounts.mail.fields = Some(MailFields {
            address: field("you@gmail.com", &gmail.address, window, cx),
            host: field("Host in your ssh config", &gmail.host, window, cx),
            program: field("gmailcli (looked up on the PATH there)", &gmail.program, window, cx),
        });
    }

    /// A field of the Mail card by its name, for a test that types into it.
    #[cfg(test)]
    pub(crate) fn mail_field(&self, name: &str) -> Entity<InputState> {
        let fields = self.accounts.mail.fields.as_ref().expect("the section was drawn");
        match name {
            "gmail-address" => &fields.address,
            "gmail-host" => &fields.host,
            "gmail-program" => &fields.program,
            other => panic!("no field {other}"),
        }
        .clone()
    }

    pub(crate) fn gmail_draft(&self, cx: &Context<Self>) -> Option<GmailSaved> {
        let fields = self.accounts.mail.fields.as_ref()?;
        Some(GmailSaved {
            address: typed(&fields.address, cx),
            // A host typed and then switched away from is not kept.
            host: if self.accounts.mail.over_ssh { typed(&fields.host, cx) } else { String::new() },
            program: typed(&fields.program, cx),
        })
    }

    /// Asks `gmailcli` which account the browser is signed in to, with what is typed. Keeps nothing. It takes several seconds.
    pub(crate) fn test_gmail(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.gmail_draft(cx) else { return };
        let services = accounts::services(cx);
        self.accounts.mail.check = Checked::Checking;
        let work = cx.background_spawn(async move {
            accounts::connect_gmail(&services, &draft)
                .map(|(_, address)| address)
                .map_err(|e| accounts::plain_words(Kind::Gmail, &e))
        });
        self.accounts.mail._task = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.mail.check = Checked::of(outcome);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Keeps the Gmail facts and has the provider built.
    pub(crate) fn save_gmail(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.gmail_draft(cx) else { return };
        let services = accounts::services(cx);
        self.accounts.mail.check = Checked::Checking;
        let asked = draft.clone();
        let work = cx.background_spawn(async move {
            accounts::connect_gmail(&services, &asked)
                .map(|(_, address)| address)
                .map_err(|e| accounts::plain_words(Kind::Gmail, &e))
        });
        self.accounts.mail._task = Some(cx.spawn(async move |this, cx| {
            let found = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.saved.gmail = Some(draft);
                pane.accounts.mail.check = Checked::of(found);
                pane.accounts_changed(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_gmail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.accounts.saved.gmail = None;
        self.accounts.mail.check = Checked::Unchecked;
        self.accounts.mail.over_ssh = false;
        if let Some(fields) = &self.accounts.mail.fields {
            for field in [&fields.address, &fields.host, &fields.program] {
                field.update(cx, |field, cx| field.set_value("", window, cx));
            }
        }
        self.accounts_changed(cx);
        cx.notify();
    }

    pub(super) fn gmail_card(&self, rows: &Rows, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let page: &AccountsPage = &self.accounts;
        let Some(fields) = &page.mail.fields else { return div().into_any_element() };
        let this = cx.entity().downgrade();
        let row = &rows.gmail;
        let saved = page.saved.gmail.as_ref();
        let detail = match (row, saved) {
            (Row::Connected(_), Some(gmail)) | (_, Some(gmail)) => gmail.address.clone(),
            _ => "Mail from your Gmail, over gmailcli".to_string(),
        };
        let place = {
            let this = this.clone();
            Segmented::new(
                "gmail-where",
                [Segment::new("On this machine").debug_name("gmail-here"), Segment::new("Over SSH").debug_name("gmail-ssh")],
                usize::from(page.mail.over_ssh),
            )
            .on_change(move |i, _, cx| {
                this.update(cx, |p, cx| {
                    p.accounts.mail.over_ssh = i == 1;
                    cx.notify();
                })
                .ok();
            })
        };
        let (test, save, forget) = (this.clone(), this.clone(), this);
        card("gmail", theme)
            .w_full()
            .child(card_head_of(icon_tile(IconName::Mail, theme), "Gmail", &detail, false, badge_of(row, saved.is_some()), theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(labelled("Address", "gmail-address", &fields.address, theme))
                    .child(div().flex().flex_col().gap(px(4.)).child(note("Where gmailcli runs", theme)).child(div().flex().child(place)))
                    .when(page.mail.over_ssh, |d| d.child(labelled("Host", "gmail-host", &fields.host, theme)))
                    .child(labelled("Path of gmailcli", "gmail-program", &fields.program, theme))
                    .child(note("Uses the browser that gmailcli drives, signed in to Gmail. No key is kept here. Mail is read only for now: you cannot send, label or delete from Atelier.", theme))
                    .children(row_note(Kind::Gmail, row, &page.mail.check, theme))
                    .children(check_line(&page.mail.check, theme))
                    .child(actions(
                        GMAIL,
                        &page.mail.check,
                        saved.is_some(),
                        move |_, cx| {
                            test.update(cx, |p, cx| p.test_gmail(cx)).ok();
                        },
                        move |_, cx| {
                            save.update(cx, |p, cx| p.save_gmail(cx)).ok();
                        },
                        move |window, cx| {
                            forget.update(cx, |p, cx| p.forget_gmail(window, cx)).ok();
                        },
                    )),
            )
            .into_any_element()
    }
}
