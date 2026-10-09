//! The Mail view of the shell: the mail accounts and their mailboxes in the sidebar, and the pane that shows the open mailbox's
//! threads and the open thread. The pane is one for the whole window (the accounts are the person's, not a project's) and is made
//! the first time the view is in front. What it shows is written once against the `MailProvider` trait, so a new provider needs
//! nothing here.
use atelier_capabilities::mail::Role;
use atelier_ui::{
    Icon, IconName,
    project_badge::{ProjectBadge, fallback_color},
    scale::px,
    theme::ActiveTheme,
    typography::TextSize,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Focusable, Window, div};

use super::{
    lens::{nav_heading, nav_row},
    structs::{OpenSettings, Shell},
    view::ShellView,
};
use crate::{
    capability_hub::CapabilityHub,
    mail::pane::{Account, MailPane, MailPaneEvent, Problem},
};

impl Shell {
    /// Shows the Mail view.
    pub(super) fn show_mail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_view(ShellView::Mail, window, cx);
        self.ensure_mail(window, cx);
        if let Some(pane) = &self.mail {
            pane.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    /// Makes the pane when there is none, and gives it the app's mail accounts as the hub has them now: one added since the last
    /// look shows, and the same accounts change nothing.
    pub(super) fn ensure_mail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = match self.mail.clone() {
            Some(pane) => pane,
            None => {
                let me = cx.try_global::<CapabilityHub>().map(CapabilityHub::person).unwrap_or_else(|| atelier_capabilities::Actor::person("me", "me"));
                let pane = cx.new(|cx| MailPane::new(me, window, cx));
                let subscription = cx.subscribe_in(&pane, window, |this: &mut Self, _, event: &MailPaneEvent, window, cx| match event {
                    MailPaneEvent::OpenSettings => this.open_settings(&OpenSettings, window, cx),
                });
                self._subscriptions.push(subscription);
                self.mail = Some(pane.clone());
                pane
            }
        };
        let providers = cx.try_global::<CapabilityHub>().map(CapabilityHub::mail_providers).unwrap_or_default();
        pane.update(cx, |pane, cx| pane.set_providers(providers, cx));
    }

    /// The main area of the Mail view: the pane, a panels' gap from the sidebar, as the Messages pane is.
    pub(super) fn mail_main(&self) -> AnyElement {
        match &self.mail {
            Some(pane) => div().debug_selector(|| "mail-view".into()).size_full().pl(px(super::types::PANE_GAP)).pr(px(8.)).child(pane.clone()).into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// The Mail view's sidebar: each account with its mailboxes and their unread counts. A press on a mailbox opens it.
    pub(super) fn mail_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(pane) = self.mail.clone() else { return div().into_any_element() };
        let theme = cx.theme().clone();
        let state = pane.read(cx);
        let open = state.open_mailbox_ref().map(|(at, mailbox)| (at, mailbox.clone()));
        let mut column = div()
            .debug_selector(|| "mail-sidebar".into())
            .id("mail-sidebar")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .gap(px(1.))
            .p(px(6.));
        for (at, account) in state.accounts().iter().enumerate() {
            column = column.child(account_heading(at, account, cx));
            if let Some(words) = account_note(account) {
                column = column.child(div().px(px(8.)).pb(px(4.)).text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(words));
            }
            // The mailboxes the person made have a heading only where there are system ones to tell them from.
            let labels = account.boxes.iter().any(|b| b.role == Role::Custom);
            let system = account.boxes.iter().any(|b| b.role != Role::Custom);
            for (n, mailbox) in account.boxes.iter().enumerate() {
                if labels && system && mailbox.role == Role::Custom && account.boxes.get(n.wrapping_sub(1)).is_none_or(|b| b.role != Role::Custom) {
                    column = column.child(nav_heading("Labels", cx));
                }
                let on = open.as_ref().is_some_and(|(a, reference)| *a == at && *reference == mailbox.reference);
                let reference = mailbox.reference.clone();
                let pane = pane.downgrade();
                let count = (mailbox.unread > 0).then_some(mailbox.unread as usize);
                let item = nav_row(format!("mail-box-{at}-{n}"), on, mailbox_mark(mailbox.role, cx), mailbox.name.clone(), count, cx)
                    .on_click(move |_, window, cx| drop(pane.update(cx, |p, cx| p.open_mailbox(at, &reference, window, cx))));
                column = column.child(item);
            }
        }
        if state.accounts().is_empty() {
            column = column.child(div().px(px(8.)).pt(px(10.)).text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("No accounts yet."));
        }
        column.into_any_element()
    }
}

/// The account's heading: its provider's letter on a colour of its own, and `Gmail · alex@example.com`.
fn account_heading(at: usize, account: &Account, cx: &gpui_kit::App) -> AnyElement {
    let theme = cx.theme();
    let name = account.choice.name();
    let letter = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    div()
        .debug_selector(move || format!("mail-account-{at}"))
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(TextSize::Xs.font_size())
        .text_color(theme.muted_foreground)
        .child(ProjectBadge::new(letter, fallback_color(&account.choice.provider)))
        .child(div().min_w_0().truncate().child(account.choice.words()))
        .into_any_element()
}

/// What stops an account's mailboxes from showing, in a few words, when something does.
fn account_note(account: &Account) -> Option<gpui_kit::SharedString> {
    match (account.problem, &account.failed) {
        (Some(Problem::Offline), _) => Some("Offline".into()),
        (Some(Problem::Wait(ms)), _) => Some(format!("Try again in {} s", ms.div_ceil(1000)).into()),
        (Some(Problem::SignedOut), _) => Some("Not signed in".into()),
        (None, Some(why)) => Some(why.clone()),
        (None, None) => None,
    }
}

/// The mark in front of a mailbox, by what it is for.
fn mailbox_mark(role: Role, cx: &gpui_kit::App) -> AnyElement {
    let name = match role {
        Role::Inbox => IconName::Download,
        Role::Sent => IconName::ArrowOutward,
        Role::Drafts => IconName::Draft,
        Role::Trash => IconName::Delete,
        Role::Spam => IconName::Block,
        Role::Archive => IconName::Archive,
        Role::Custom => IconName::Folder,
    };
    Icon::new(name).size(px(13.)).color(cx.theme().muted_foreground).into_any_element()
}
