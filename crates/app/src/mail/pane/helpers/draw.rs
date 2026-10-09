use atelier_ui::{
    Button, ButtonSize, ButtonVariant, Field, Icon, IconName,
    scale::px,
    theme::{ActiveTheme, Theme, radius},
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, component::input::Textarea, div,
    prelude::FluentBuilder,
};

use super::super::{
    structs::Compose,
    types::{OpenAddress, Press, Problem},
};
use crate::mail::map::{MessageView, ThreadRow};

/// The banner of a problem the pane waits out: offline has Retry, a wait says how long. `None` for the problem that takes the
/// whole pane.
pub fn banner(
    problem: Problem,
    retry: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> Option<AnyElement> {
    let words = match problem {
        Problem::Offline => "There is no connection.".to_string(),
        Problem::Wait(ms) => format!("Too many requests. Try again in {} s.", ms.div_ceil(1000)),
        Problem::SignedOut => return None,
    };
    let row = div()
        .id("mail-banner")
        .debug_selector(|| "mail-banner".into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .pb(px(8.))
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(words),
        );
    let row = if problem == Problem::Offline {
        row.child(
            Button::new("mail-retry")
                .debug_name("mail-retry")
                .label("Retry")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| retry(window, cx)),
        )
    } else {
        row
    };
    Some(row.into_any_element())
}

/// The empty state of an account the reader is not signed in to, with a way to Settings.
pub fn signed_out(
    account: &str,
    open_settings: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    state(
        "mail-signed-out",
        format!("Sign in to {account} to see its mail."),
        "mail-sign-in",
        "Open settings",
        open_settings,
        theme,
    )
}

/// The empty state of an app with no mail account.
pub fn no_account(
    open_settings: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    state(
        "mail-empty",
        "No mail account is connected.".to_string(),
        "mail-open-settings",
        "Open settings",
        open_settings,
        theme,
    )
}

fn state(
    name: &'static str,
    words: String,
    button: &'static str,
    label: &'static str,
    press: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    div()
        .id(name)
        .debug_selector(move || name.into())
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .px(px(24.))
        .child(div().text_color(theme.muted_foreground).child(words))
        .child(
            Button::new(button)
                .debug_name(button)
                .label(label)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| press(window, cx)),
        )
        .into_any_element()
}

/// One muted line in the middle of a card: what it waits for, or what went wrong.
pub fn say(words: SharedString, theme: &Theme) -> AnyElement {
    div()
        .debug_selector(|| "mail-line".into())
        .flex_1()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(24.))
        .text_color(theme.muted_foreground)
        .child(words)
        .into_any_element()
}

/// A text button of the reading pane's head: the name of what it does, and nothing else.
pub fn action(id: &'static str, label: &'static str, press: Press) -> AnyElement {
    Button::new(id)
        .debug_name(id)
        .label(label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Sm)
        .on_click(move |_, window, cx| press(window, cx))
        .into_any_element()
}

/// Row `ix` of the thread list: who, how many messages, the age, the subject, the snippet. An unread thread is bold and has a
/// dot; a starred one has its star; a thread with a file has the clip.
pub fn thread_item(ix: usize, row: &ThreadRow, open: bool, press: Press, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let weight = if row.unread {
        FontWeight::SEMIBOLD
    } else {
        FontWeight::NORMAL
    };
    let name = format!("mail-thread-{ix}");
    let selector = name.clone();
    let dot = format!("mail-unread-{ix}");
    let star = format!("mail-star-{ix}");
    let clip = format!("mail-clip-{ix}");
    let who_line = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().flex_none().size(px(7.)).rounded_full().when(row.unread, |d| {
            d.debug_selector(move || dot.clone()).bg(theme.accent)
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(TextSize::Sm.font_size())
                .font_weight(weight)
                .child(row.who.clone()),
        )
        .when(row.count > 1, |d| {
            d.child(
                div()
                    .flex_none()
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(format!("({})", row.count)),
            )
        })
        .when(row.attachment, |d| {
            d.child(
                div()
                    .debug_selector(move || clip.clone())
                    .flex_none()
                    .child(Icon::new(IconName::AttachFile).size(px(12.)).color(muted)),
            )
        })
        .child(
            div()
                .flex_none()
                .text_size(TextSize::Xs.font_size())
                .text_color(muted)
                .child(row.time.clone()),
        );
    let subject_line = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(13.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(TextSize::Sm.font_size())
                .font_weight(weight)
                .child(row.subject.clone()),
        )
        .when(row.starred, |d| {
            d.child(
                div()
                    .debug_selector(move || star.clone())
                    .flex_none()
                    .child(Icon::new(IconName::StarFilled).size(px(12.)).color(theme.accent)),
            )
        });
    let snippet_line = div()
        .pl(px(13.))
        .truncate()
        .text_size(TextSize::Xs.font_size())
        .text_color(muted)
        .child(row.snippet.clone());
    div()
        .w_full()
        .px(px(6.))
        .py(px(1.))
        .child(
            div()
                .id(SharedString::from(name))
                .debug_selector(move || selector.clone())
                .flex()
                .flex_col()
                .gap(px(2.))
                .px(px(8.))
                .py(px(7.))
                .rounded(radius::md())
                .cursor_pointer()
                .when(open, |d| d.bg(theme.card_strong))
                .when(!open, |d| d.hover(|s| s.bg(theme.card_strong.opacity(0.6))))
                .on_click(move |_, window, cx| press(window, cx))
                .child(who_line)
                .child(subject_line)
                .child(snippet_line),
        )
        .into_any_element()
}

/// The row after the last thread that reads the page after it.
pub fn load_more(loading: bool, load: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
    div()
        .id("mail-load-more-row")
        .w_full()
        .flex()
        .flex_none()
        .justify_center()
        .py(px(6.))
        .child(
            Button::new("mail-load-more")
                .debug_name("mail-load-more")
                .label(if loading { "Loading…" } else { "Load more" })
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| load(window, cx)),
        )
        .into_any_element()
}

/// One message: who wrote it and to whom, when, its text, the files it carries and the addresses in it. The text is drawn as
/// the characters it is: the tags of a message are letters on the screen. `whole` is the reader's wish to see all of a long
/// body. An address is a line of text, and a press on it only asks `open_address`; the pane shows it and waits for a yes.
pub fn message_card(
    ix: usize,
    view: &MessageView,
    whole: bool,
    show_all: Press,
    open_address: OpenAddress,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let small = TextSize::Xs.font_size();
    let from = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(TextSize::Sm.font_size())
                .font_weight(FontWeight::MEDIUM)
                .child(
                    view.from_name
                        .clone()
                        .unwrap_or_else(|| view.from_address.clone()),
                ),
        )
        .when(view.from_name.is_some(), |d| {
            // The address is always shown beside a name, so a name cannot pose as another sender.
            d.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(small)
                    .text_color(muted)
                    .child(format!("<{}>", view.from_address)),
            )
        })
        .child(div().flex_1())
        .when(view.unread, |d| {
            d.child(
                div()
                    .debug_selector(move || format!("mail-message-unread-{ix}"))
                    .flex_none()
                    .size(px(7.))
                    .rounded_full()
                    .bg(theme.accent),
            )
        })
        .child(
            div()
                .flex_none()
                .text_size(small)
                .text_color(muted)
                .child(view.time.clone()),
        );
    let cut = view.cut.is_some() && !whole;
    // A cut body ends in an ellipsis, so it does not read as the whole of what was written.
    let shown: SharedString = match cut {
        true => format!("{}…", view.shown(whole)).into(),
        false => view.shown(whole).clone(),
    };
    let links = (!view.links.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(small).text_color(muted).child("Addresses in this message"))
            .children(view.links.iter().enumerate().map(|(n, link)| {
                let (address, open) = (link.to_string(), open_address.clone());
                div()
                    .id(ElementId::Name(format!("mail-link-{ix}-{n}").into()))
                    .debug_selector(move || format!("mail-link-{ix}-{n}"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(24.))
                    .px(px(6.))
                    .rounded(radius::md())
                    .cursor_pointer()
                    .text_size(small)
                    .hover(|s| s.bg(theme.card_strong.opacity(0.6)))
                    .on_click(move |_, window, cx| open(&address, window, cx))
                    .child(Icon::new(IconName::Link).size(px(12.)).color(muted))
                    .child(div().min_w_0().truncate().child(link.clone()))
            }))
    });
    let files = (!view.attachments.is_empty()).then(|| {
        div().flex().flex_wrap().gap(px(4.)).children(
            view.attachments.iter().enumerate().map(|(n, file)| {
                div()
                    .debug_selector(move || format!("mail-file-{ix}-{n}"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(radius::md())
                    .bg(theme.card_strong)
                    .text_size(small)
                    .child(Icon::new(IconName::AttachFile).size(px(14.)).color(muted))
                    .child(file.name.clone())
                    .child(div().text_color(muted).child(file.detail.clone()))
            }),
        )
    });
    div()
        .debug_selector(move || format!("mail-message-{ix}"))
        .flex()
        .flex_col()
        .gap(px(6.))
        .px(px(12.))
        .py(px(10.))
        .child(from)
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().min_w_0().truncate().text_size(small).text_color(muted).child(view.to.clone()))
                .children(
                    view.cc
                        .clone()
                        .map(|cc| div().min_w_0().truncate().text_size(small).text_color(muted).child(cc)),
                ),
        )
        .child(
            div()
                .debug_selector(move || format!("mail-body-{ix}"))
                .text_size(TextSize::Sm.font_size())
                .line_height(px(22.))
                .child(shown),
        )
        .when(cut, |d| {
            d.child(
                div().flex().ml(px(-8.)).child(
                    div().debug_selector(move || format!("mail-show-all-{ix}")).child(
                        Button::new(ElementId::Name(format!("mail-show-all-{ix}").into()))
                            .label("Show all")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(move |_, window, cx| show_all(window, cx)),
                    ),
                ),
            )
        })
        .children(files)
        .children(links)
        .into_any_element()
}

/// The question over a press on an address: where it goes, written out, and the two answers.
pub fn link_bar(address: &str, open: Press, cancel: Press, theme: &Theme) -> AnyElement {
    div()
        .id("mail-link-bar")
        .debug_selector(|| "mail-link-bar".into())
        .flex_none()
        .mx(px(12.))
        .mb(px(8.))
        .p(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .rounded(radius::md())
        .bg(theme.card_strong)
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child("This address opens in your browser. Check where it goes."),
        )
        .child(
            div()
                .debug_selector(|| "mail-link-address".into())
                .text_size(TextSize::Sm.font_size())
                .child(address.to_string()),
        )
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(
                    Button::new("mail-link-open")
                        .debug_name("mail-link-open")
                        .label("Open in browser")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, window, cx| open(window, cx)),
                )
                .child(
                    Button::new("mail-link-cancel")
                        .debug_name("mail-link-cancel")
                        .label("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, window, cx| cancel(window, cx)),
                ),
        )
        .into_any_element()
}

/// The reply box under the thread: to whom it goes, the text, and the buttons the provider allows. A button the provider does
/// not list is not drawn.
pub fn composer(compose: &Compose, theme: &Theme) -> AnyElement {
    let Compose {
        to,
        status,
        input,
        focus,
        editable,
        busy,
        empty,
        save,
        send,
    } = compose;
    let field = Field::new(
        focus.clone(),
        Textarea::new(input)
            .appearance(false)
            .disabled(!*editable || *busy),
    )
    .radius(radius::md())
    .padding(px(0.));
    let button = |id: &'static str, label: &'static str, variant: ButtonVariant, press: &Press| {
        let press = press.clone();
        Button::new(id)
            .debug_name(id)
            .label(label)
            .variant(variant)
            .size(ButtonSize::Sm)
            .disabled(*empty || *busy)
            .on_click(move |_, window, cx| press(window, cx))
    };
    div()
        .id("mail-composer")
        .debug_selector(|| "mail-composer".into())
        .flex_none()
        .px(px(12.))
        .pb(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(radius::lg())
                .bg(theme.card_strong)
                .child(
                    div()
                        .debug_selector(|| "mail-compose-to".into())
                        .min_w_0()
                        .truncate()
                        .text_size(TextSize::Xs.font_size())
                        .text_color(theme.muted_foreground)
                        .child(to.clone()),
                )
                .child(field)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .debug_selector(|| "mail-compose-status".into())
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(TextSize::Xs.font_size())
                                .text_color(theme.muted_foreground)
                                .child(status.clone().unwrap_or_default()),
                        )
                        .children(
                            save.as_ref()
                                .map(|p| button("mail-save-draft", "Save draft", ButtonVariant::Ghost, p)),
                        )
                        .children(
                            send.as_ref()
                                .map(|p| button("mail-send", "Send", ButtonVariant::Primary, p)),
                        ),
                ),
        )
        .into_any_element()
}
