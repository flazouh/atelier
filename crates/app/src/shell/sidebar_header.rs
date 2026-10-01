//! The head of the sidebar: "Projects" with a button to filter the sessions and a button to add a project, and under
//! them a box that narrows the sessions by title. The filter's choices are what the sidebar lists
//! ([`beui::sidebar_filter::SessionFilter`]); the add button opens the two ways to bring in a project.
use super::*;
use beui::{
    menu::{self, Choice, Entry, Menu, MenuItem, MenuLook, Origin},
    sidebar_filter::{SessionFilter, describe, hidden_by},
};
use gpui_kit::component::input::{Input, InputEvent, InputState};

impl Shell {
    /// Makes the box that narrows the sessions, the first time there is a window; a change in it narrows the list.
    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter sessions"));
        self._filter_input = Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.push_sidebar(cx);
            }
        }));
        self.filter_input = Some(input);
    }

    /// A button with a menu hung under it, drawn open while `open`.
    fn head_button(&self, id: &'static str, icon: beui::IconName, tip: SharedString, lit: bool, open: bool, menu: Option<AnyElement>, press: impl Fn(&mut Window, &mut gpui_kit::App) + 'static) -> AnyElement {
        div()
            .relative()
            .child(
                Button::new(id)
                    .debug_name(id)
                    .icon(icon)
                    .variant(if lit { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .size(ButtonSize::IconSm)
                    .tooltip(tip)
                    .open(open)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        press(window, cx)
                    }),
            )
            .children(menu)
            .into_any_element()
    }

    pub(super) fn sidebar_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let this = cx.entity().downgrade();
        let query = self.filter_input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        let filter = self.session_filter;

        let filter_menu = self.filter_menu.then(|| {
            let hidden = hidden_by(&self.all_projects, SessionFilter::Active);
            let entries: Vec<Entry> = SessionFilter::ALL
                .into_iter()
                .map(|choice| {
                    let pick = this.clone();
                    let words = if choice == SessionFilter::Archived && hidden > 0 { format!("{} ({hidden})", choice.words()) } else { choice.words().to_string() };
                    Entry::from(
                        MenuItem::new(words)
                            .debug_name(choice.row())
                            .choice(Choice::Radio(choice == filter))
                            .on_select(move |_, cx| drop(pick.update(cx, |s, cx| s.choose_filter(choice, cx)))),
                    )
                })
                .collect();
            let close = this.clone();
            Popover::new("filter-menu-popover")
                .open(true)
                .hang(Hang::Right(0., 30.))
                .keep_focus()
                .height(menu::height_in(MenuLook::PROJECT, SessionFilter::ALL.len()))
                .on_close(move |_, cx| drop(close.update(cx, |s, cx| {
                    s.filter_menu = false;
                    cx.notify();
                })))
                .child(Menu::new("filter-menu-panel", entries).look(MenuLook::PROJECT).origin(Origin::TopRight))
                .into_any_element()
        });
        let add_menu = self.add_menu.then(|| {
            let (folder, remote, close) = (this.clone(), this.clone(), this.clone());
            let entries: Vec<Entry> = vec![
                Entry::from(
                    MenuItem::new("Open folder…")
                        .debug_name("add-folder")
                        .cap(keys::cap("⌘o"))
                        .on_select(move |window, cx| drop(folder.update(cx, |s, cx| {
                            s.add_menu = false;
                            s.open_folder(&OpenFolder, window, cx)
                        }))),
                ),
                Entry::from(
                    MenuItem::new("Open over SSH…")
                        .debug_name("add-ssh")
                        .cap(keys::cap("⌘⇧o"))
                        .on_select(move |window, cx| drop(remote.update(cx, |s, cx| {
                            s.add_menu = false;
                            s.open_ssh_form(&OpenRemote, window, cx)
                        }))),
                ),
            ];
            Popover::new("add-menu-popover")
                .open(true)
                .hang(Hang::Right(0., 30.))
                .keep_focus()
                .height(menu::height_in(MenuLook::PROJECT, 2))
                .on_close(move |_, cx| drop(close.update(cx, |s, cx| {
                    s.add_menu = false;
                    cx.notify();
                })))
                .child(Menu::new("add-menu-panel", entries).look(MenuLook::PROJECT).origin(Origin::TopRight))
                .into_any_element()
        });

        let (toggle_filter, toggle_add) = (this.clone(), this.clone());
        let lit = !filter.is_default() || !query.trim().is_empty();
        let buttons = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(self.head_button("filter-button", beui::IconName::FilterList, describe(filter, &query), lit, self.filter_menu, filter_menu, move |_, cx| {
                drop(toggle_filter.update(cx, |s, cx| {
                    s.filter_menu = !s.filter_menu;
                    s.add_menu = false;
                    cx.notify();
                }))
            }))
            .child(self.head_button("add-project", beui::IconName::Add, "Add a project".into(), false, self.add_menu, add_menu, move |_, cx| {
                drop(toggle_add.update(cx, |s, cx| {
                    s.add_menu = !s.add_menu;
                    s.filter_menu = false;
                    cx.notify();
                }))
            }));
        let search = self.filter_input.as_ref().map(|input| {
            div()
                .debug_selector(|| "session-filter".into())
                .mx(px(8.))
                .mb(px(4.))
                .rounded(radius::LG)
                .bg(theme.card)
                .child(Input::new(input).appearance(false).px(px(8.)).text_size(TextSize::Sm.font_size()))
        });
        div()
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.))
                    .pl(px(16.))
                    .pr(px(8.))
                    .child(div().text_size(TextSize::Xs.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).text_color(theme.muted_foreground).child("Projects"))
                    .child(buttons),
            )
            .children(search)
            .into_any_element()
    }
}
