//! The Messages view of the shell: the chat accounts and their channels in the sidebar, and the pane that shows the open channel.
//! The pane is one for the whole window (the accounts are the person's, not a project's) and is made the first time the view is in
//! front. What it shows is written once against the `MessagingProvider` trait, so a new provider needs nothing here.
use atelier_ui::{
    Icon, IconName,
    project_badge::{ProjectBadge, fallback_color},
    scale::px,
    theme::ActiveTheme,
    typography::TextSize,
};
use atelier_capabilities::messaging::ChannelKind;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Focusable, Window, div};

use super::{
    lens::{nav_heading, nav_row},
    structs::Shell,
    view::ShellView,
};
use crate::{
    capability_hub::CapabilityHub,
    messages::{
        map::{Group, Row},
        pane::{Account, MessagesEvent, MessagesPane},
    },
};

impl Shell {
    /// Shows the Messages view, with the focus in what the reader writes in.
    pub(super) fn show_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_view(ShellView::Messages, window, cx);
        self.ensure_messages(window, cx);
        if let Some(pane) = &self.messages {
            pane.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    /// Makes the pane when there is none, and gives it the app's messaging accounts as the hub has them now: one added since
    /// the last look shows, and the same accounts change nothing.
    pub(super) fn ensure_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = match self.messages.clone() {
            Some(pane) => pane,
            None => {
                let pane = cx.new(|cx| MessagesPane::new(window, cx));
                let subscription = cx.subscribe_in(&pane, window, |this: &mut Self, _, event: &MessagesEvent, window, cx| match event {
                    MessagesEvent::OpenAccounts => this.open_accounts(window, cx),
                });
                self._subscriptions.push(subscription);
                self.messages = Some(pane.clone());
                pane
            }
        };
        let providers = cx.try_global::<CapabilityHub>().map(CapabilityHub::messaging_providers).unwrap_or_default();
        pane.update(cx, |pane, cx| pane.set_providers(providers, cx));
    }

    /// The hub changed: an open Messages pane takes its accounts as they are now. The same accounts change nothing.
    pub(super) fn refresh_messages(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.messages.clone() else { return };
        let providers = cx.try_global::<CapabilityHub>().map(CapabilityHub::messaging_providers).unwrap_or_default();
        pane.update(cx, |pane, cx| pane.set_providers(providers, cx));
    }

    /// The main area of the Messages view: the pane, a panels' gap from the sidebar, as the Tasks pane is.
    pub(super) fn messages_main(&self) -> AnyElement {
        match &self.messages {
            Some(pane) => div().debug_selector(|| "messages-view".into()).size_full().pl(px(super::types::PANE_GAP)).pr(px(8.)).child(pane.clone()).into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// The Messages view's sidebar: each account with its channels and direct messages. A press on a channel opens it.
    pub(super) fn messages_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(pane) = self.messages.clone() else { return div().into_any_element() };
        let theme = cx.theme().clone();
        let state = pane.read(cx);
        let open = state.open_channel_ref().map(|(at, channel)| (at, channel.clone()));
        let mut column = div()
            .debug_selector(|| "messages-sidebar".into())
            .id("messages-sidebar")
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
            // The groups have headings only where there is more than one to tell apart.
            let both = account.group(Group::Channels).next().is_some() && account.group(Group::Direct).next().is_some();
            for (group, heading) in [(Group::Channels, "Channels"), (Group::Direct, "Direct messages")] {
                let rows: Vec<(usize, &Row)> = account.rows.iter().enumerate().filter(|(_, r)| r.group == group).collect();
                if both && !rows.is_empty() {
                    column = column.child(nav_heading(heading, cx));
                }
                for (n, row) in rows {
                    let on = open.as_ref().is_some_and(|(a, channel)| *a == at && *channel == row.reference);
                    let channel = row.reference.clone();
                    let pane = pane.downgrade();
                    let dot = row.unread.then(|| {
                        div().debug_selector(move || format!("messages-unread-{at}-{n}")).flex_none().size(px(7.)).rounded_full().bg(theme.accent)
                    });
                    let item = nav_row(format!("messages-channel-{at}-{n}"), on, channel_mark(row, cx), row.name.clone(), None, cx)
                        .on_click(move |_, _, cx| drop(pane.update(cx, |p, cx| p.open_channel(at, &channel, cx))));
                    column = column.child(item.children(dot));
                }
            }
        }
        if state.accounts().is_empty() {
            column = column.child(div().px(px(8.)).pt(px(10.)).text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("No accounts yet."));
        }
        column.into_any_element()
    }
}

/// The account's heading: its provider's letter on a colour of its own, and `Slack · acme`.
fn account_heading(at: usize, account: &Account, cx: &gpui_kit::App) -> AnyElement {
    let theme = cx.theme();
    let name = account.choice.name();
    let letter = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    div()
        .debug_selector(move || format!("messages-account-{at}"))
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

/// What stops an account's channels from showing, in a few words, when something does.
fn account_note(account: &Account) -> Option<gpui_kit::SharedString> {
    use crate::messages::pane::Problem;
    match (account.problem, &account.failed) {
        (Some(Problem::Offline), _) => Some("Offline".into()),
        (Some(Problem::Wait(ms)), _) => Some(format!("Try again in {} s", ms.div_ceil(1000)).into()),
        (Some(Problem::SignedOut), _) => Some("Not signed in".into()),
        (None, Some(why)) => Some(why.clone()),
        (None, None) => None,
    }
}

/// The mark in front of a channel: `#` for a public one, a lock for a private one, `@` for a direct chat.
fn channel_mark(row: &Row, cx: &gpui_kit::App) -> AnyElement {
    match row.kind {
        ChannelKind::Public => div().child("#").into_any_element(),
        ChannelKind::Private => Icon::new(IconName::Lock).size(px(12.)).color(cx.theme().muted_foreground).into_any_element(),
        ChannelKind::Dm | ChannelKind::GroupDm => div().child("@").into_any_element(),
    }
}
