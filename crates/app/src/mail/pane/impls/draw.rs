use std::rc::Rc;

use atelier_capabilities::mail::{MailOperation, Role};
use atelier_ui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, IconName, TextInput,
    menu::{self, Choice, Entry, Menu, MenuItem, MenuLook},
    popover::{Hang, Popover},
    scale::px,
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ListState, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, list,
};

use super::super::{
    helpers::{
        action, banner, composer, link_bar, load_more, message_card, say, thread_item,
    },
    structs::MailPane,
    types::{LIST_WIDTH, Load, Menu as OpenMenu, OpenAddress, Press},
};

impl MailPane {
    /// The open mailbox's words, or `Mail`.
    fn title(&self) -> SharedString {
        self.mailbox
            .as_ref()
            .and_then(|m| self.account()?.boxes.iter().find(|b| b.reference == *m))
            .map_or_else(|| "Mail".into(), |b| b.name.clone())
    }

    /// The left card: the mailbox's name, the search, and the threads.
    pub(in crate::mail::pane) fn list_card(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let pane = cx.entity();
        let header = div()
            .id("mail-header")
            .debug_selector(|| "mail-header".into())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(12.))
            .child(
                div()
                    .debug_selector(|| "mail-title".into())
                    .min_w_0()
                    .truncate()
                    .text_size(TextSize::Sm.font_size())
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(self.title()),
            )
            .children(self.account().filter(|_| self.mailbox.is_some()).map(|a| {
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(TextSize::Xs.font_size())
                    .text_color(theme.muted_foreground)
                    .child(a.choice.words())
            }));
        // A provider with no search has no field, and nothing in its place.
        let search = self.can(MailOperation::Search).then(|| {
            div()
                .debug_selector(|| "mail-search".into())
                .flex_none()
                .px(px(10.))
                .pb(px(8.))
                .child(
                    TextInput::new("mail-search-field", &self.search)
                        .debug_name("mail-search-field")
                        .left_icon(IconName::Search),
                )
        });
        let notice = self.effective_problem().and_then(|problem| {
            let pane = pane.clone();
            banner(problem, move |_, cx| pane.update(cx, |p, cx| p.reload(cx)), &theme)
        });
        let said = self.said.clone().filter(|_| self.open.is_none()).map(|said| self.said_line(said, &theme));
        let body = self.threads_body(cx);
        div()
            .id("mail-list")
            .debug_selector(|| "mail-list".into())
            .flex()
            .flex_col()
            .flex_none()
            .w(px(LIST_WIDTH))
            .h_full()
            .rounded(radius::lg())
            .bg(theme.card)
            .child(header)
            .children(search)
            .children(said)
            .children(notice)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn said_line(&self, said: SharedString, theme: &atelier_ui::theme::Theme) -> AnyElement {
        div()
            .debug_selector(|| "mail-said".into())
            .flex_none()
            .px(px(12.))
            .pb(px(8.))
            .text_size(TextSize::Xs.font_size())
            .text_color(theme.danger)
            .child(said)
            .into_any_element()
    }

    /// The threads of the open mailbox, as a list that lays out the rows on screen; or the words of what stops it.
    fn threads_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let problem = self.effective_problem();
        let Some(mailbox) = self.mailbox.clone() else {
            let words = match (self.account(), problem) {
                (Some(a), _) if a.failed.is_some() => a.failed.clone().unwrap_or_default(),
                (Some(a), _) if a.reading => "Reading the mailboxes…".into(),
                (_, Some(_)) => "The mailboxes could not be read.".into(),
                _ => "This account has no mailboxes.".into(),
            };
            return say(words, &theme);
        };
        match &self.threads_load {
            Load::Loading => return say("Reading the mail…".into(), &theme),
            Load::Failed(why) => return say(why.clone(), &theme),
            Load::Ready => {}
        }
        if self.threads.is_empty() {
            let words = if problem.is_some() {
                "The mail could not be read."
            } else if !self.query.is_empty() {
                "No thread matches."
            } else {
                "No mail here."
            };
            return say(words.into(), &theme);
        }
        let (rows, open, more, state, pane): (_, _, _, ListState, _) = (
            self.threads.clone(),
            self.open.clone(),
            self.tail.then_some(self.loading_more),
            self.list.clone(),
            cx.entity().downgrade(),
        );
        let _ = mailbox;
        list(state, move |ix, _, cx| match rows.get(ix) {
            Some(row) => {
                let (pane, thread) = (pane.clone(), row.reference.clone());
                let press: Press = Rc::new(move |window, cx| {
                    drop(pane.update(cx, |p, cx| p.open_thread(&thread, window, cx)))
                });
                thread_item(ix, row, open.as_ref() == Some(&row.reference), press, cx)
            }
            None => match more {
                Some(loading) => {
                    let pane = pane.clone();
                    load_more(loading, move |_, cx| {
                        drop(pane.update(cx, |p, cx| p.load_more(cx)))
                    })
                }
                None => div().into_any_element(),
            },
        })
        .size_full()
        .into_any_element()
    }

    /// The right card: the open thread's head and what the provider lets the reader do with it, its messages, and the reply box.
    pub(in crate::mail::pane) fn reading_card(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let card = div()
            .id("mail-reading")
            .debug_selector(|| "mail-reading".into())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(radius::lg())
            .bg(theme.card);
        if self.open.is_none() {
            let words = match self.threads_load {
                Load::Ready if self.threads.is_empty() => "",
                _ => "Choose a thread to read it.",
            };
            return card.child(say(words.into(), &theme)).into_any_element();
        }
        let subject: SharedString = self
            .thread
            .as_ref()
            .map(|t| t.summary.subject.clone())
            .or_else(|| {
                self.threads
                    .iter()
                    .find(|r| Some(&r.reference) == self.open.as_ref())
                    .map(|r| r.subject.to_string())
            })
            .map(|s| {
                match s.split_whitespace().collect::<Vec<_>>().join(" ") {
                    s if s.is_empty() => "(no subject)".to_string(),
                    s => s,
                }
                .into()
            })
            .unwrap_or_default();
        let header = div()
            .id("mail-reading-header")
            .debug_selector(|| "mail-reading-header".into())
            .flex()
            .flex_none()
            .items_center()
            .h(px(44.))
            .px(px(12.))
            .child(
                div()
                    .debug_selector(|| "mail-subject".into())
                    .min_w_0()
                    .truncate()
                    .text_size(TextSize::Sm.font_size())
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(subject),
            );
        let body = match &self.thread_load {
            Load::Loading => say("Reading the thread…".into(), &theme),
            Load::Failed(why) => say(why.clone(), &theme),
            Load::Ready => self.messages_column(cx),
        };
        let actions = matches!(self.thread_load, Load::Ready)
            .then(|| self.actions_row(cx))
            .flatten();
        let said = self.said.clone().map(|said| self.said_line(said, &theme));
        let link = self.link.clone().map(|address| {
            let (pane_open, pane_cancel) = (cx.entity().downgrade(), cx.entity().downgrade());
            let open: Press = Rc::new(move |_, cx| drop(pane_open.update(cx, |p, cx| p.open_link(cx))));
            let cancel: Press = Rc::new(move |_, cx| drop(pane_cancel.update(cx, |p, cx| p.cancel_link(cx))));
            link_bar(&address, open, cancel, &theme)
        });
        let compose = self.compose_view(cx).map(|c| composer(&c, &theme));
        card.child(header)
            .children(actions)
            .children(said)
            .children(link)
            .child(div().flex_1().min_h_0().child(body))
            .children(compose)
            .into_any_element()
    }

    /// The messages of the open thread, oldest first, in a column that scrolls.
    fn messages_column(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pane = cx.entity().downgrade();
        let open: OpenAddress = {
            let pane = pane.clone();
            Rc::new(move |address, _, cx| drop(pane.update(cx, |p, cx| p.ask_link(address, cx))))
        };
        let cards = self.views.iter().enumerate().map(|(ix, view)| {
            let (pane, message) = (pane.clone(), view.reference.clone());
            let show_all: Press = Rc::new(move |_, cx| {
                drop(pane.update(cx, |p, cx| p.show_all(&message, cx)))
            });
            message_card(ix, view, self.whole.contains(&view.reference), show_all, open.clone(), cx)
        });
        div()
            .id("mail-messages")
            .debug_selector(|| "mail-messages".into())
            .size_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pb(px(8.))
            .overflow_y_scroll()
            .children(cards)
            .into_any_element()
    }

    /// What the provider lets the reader do with the open thread, as a row of text buttons. A call the provider does not list has
    /// no button; a provider that lists none has no row.
    fn actions_row(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let summary = self.thread.as_ref()?.summary.clone();
        let role = self
            .mailbox
            .as_ref()
            .and_then(|m| self.account()?.boxes.iter().find(|b| b.reference == *m))
            .map(|b| b.role);
        let pane = cx.entity().downgrade();
        let press = |f: fn(&mut MailPane, &mut Context<MailPane>)| -> Press {
            let pane = pane.clone();
            Rc::new(move |_, cx| drop(pane.update(cx, |p, cx| f(p, cx))))
        };
        let mut buttons: Vec<AnyElement> = Vec::new();
        if self.can(MailOperation::Star) {
            let pane = pane.clone();
            let starred = summary.starred;
            let p: Press = Rc::new(move |_, cx| drop(pane.update(cx, |p, cx| p.star(!starred, cx))));
            buttons.push(action("mail-star", if starred { "Unstar" } else { "Star" }, p));
        }
        if self.can(MailOperation::MarkRead) {
            let pane = pane.clone();
            let unread = summary.unread > 0;
            let p: Press = Rc::new(move |_, cx| drop(pane.update(cx, |p, cx| p.mark_read(unread, cx))));
            buttons.push(action("mail-mark-read", if unread { "Mark read" } else { "Mark unread" }, p));
        }
        if self.can(MailOperation::Archive) && !matches!(role, Some(Role::Archive | Role::Trash)) {
            buttons.push(action("mail-archive", "Archive", press(|p, cx| p.archive(cx))));
        }
        if self.can(MailOperation::Trash) && role != Some(Role::Trash) {
            buttons.push(action("mail-trash", "Trash", press(|p, cx| p.trash(cx))));
        }
        if self.can(MailOperation::Move) {
            buttons.push(self.menu_button(OpenMenu::Move, "mail-move", "Move to", cx));
        }
        if self.can(MailOperation::Label) {
            buttons.push(self.menu_button(OpenMenu::Label, "mail-label", "Labels", cx));
        }
        (!buttons.is_empty()).then(|| {
            div()
                .debug_selector(|| "mail-actions".into())
                .flex()
                .flex_none()
                .flex_wrap()
                .items_center()
                .gap(px(2.))
                .px(px(6.))
                .pb(px(6.))
                .children(buttons)
                .into_any_element()
        })
    }

    /// A button that opens a small menu of the account's mailboxes: where to move the thread, or which labels it has.
    fn menu_button(
        &mut self,
        kind: OpenMenu,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pane = cx.entity().downgrade();
        let open = self.menu == Some(kind);
        let popover = open.then(|| {
            let current = self.mailbox.clone();
            let on_thread = self
                .thread
                .as_ref()
                .map(|t| t.summary.mailboxes.clone())
                .unwrap_or_default();
            let boxes: Vec<_> = self
                .account()
                .map(|a| a.boxes.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|b| match kind {
                    OpenMenu::Move => Some(&b.reference) != current.as_ref(),
                    OpenMenu::Label => b.role == Role::Custom,
                })
                .collect();
            let mut entries: Vec<Entry> = Vec::new();
            for (n, b) in boxes.iter().enumerate() {
                let (target, pane) = (b.reference.clone(), pane.clone());
                let item = MenuItem::new(b.name.clone()).debug_name(format!("{id}-{n}"));
                entries.push(Entry::from(match kind {
                    OpenMenu::Move => item.on_select(move |_, cx| {
                        drop(pane.update(cx, |p, cx| p.move_to(&target, cx)))
                    }),
                    OpenMenu::Label => {
                        let has = on_thread.contains(&b.reference);
                        item.choice(Choice::Check(has)).on_select(move |_, cx| {
                            drop(pane.update(cx, |p, cx| p.label(&target, !has, cx)))
                        })
                    }
                }));
            }
            if entries.is_empty() {
                entries.push(Entry::Label("No other mailbox".into()));
            }
            let close = pane.clone();
            Popover::new(format!("{id}-popover"))
                .open(true)
                .hang(Hang::Left(0., 30.))
                .keep_focus()
                .height(menu::height_of(MenuLook::SELECT, &entries))
                .on_close(move |_, cx| drop(close.update(cx, |p, cx| p.close_menu(cx))))
                .child(Menu::new(format!("{id}-menu"), entries).look(MenuLook::SELECT).min_width(200.))
        });
        let toggle = pane.clone();
        div()
            .relative()
            .child(
                Button::new(id)
                    .debug_name(id)
                    .label(label)
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .open(open)
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        drop(toggle.update(cx, |p, cx| p.toggle_menu(kind, cx)))
                    }),
            )
            .children(popover)
            .into_any_element()
    }
}
