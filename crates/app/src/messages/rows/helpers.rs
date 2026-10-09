use std::rc::Rc;

use atelier_capabilities::Ref;
use atelier_ui::{
    AgentText, Badge, Button, ButtonSize, ButtonVariant, Icon, IconName,
    project_badge::ProjectBadge,
    scale::px,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::FluentBuilder,
};

use super::super::map::{Body, Chip, File, Line};

/// What a press on "N replies" asks for, with the root message of the thread.
pub type OpenThread = Rc<dyn Fn(&Ref, &mut Window, &mut App)>;

/// Row `ix` of the list shown. `thread` is `None` where the provider has no threads, and then no link is drawn.
pub fn draw_line(ix: usize, line: &Line, thread: Option<OpenThread>, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let head = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .text_size(TextSize::Sm.font_size())
                .font_weight(FontWeight::MEDIUM)
                .child(line.author.clone()),
        )
        .children(line.origin.clone().map(|origin| {
            div()
                .debug_selector(move || format!("message-origin-{ix}"))
                .child(Badge::new(origin))
        }))
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(muted)
                .child(line.time.clone()),
        )
        .when(line.edited, |d| {
            d.child(
                div()
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child("edited"),
            )
        });
    let text = match &line.body {
        Body::Markdown(markdown) => AgentText::new(
            ElementId::Name(format!("message-text-{ix}").into()),
            markdown.clone(),
        )
        .into_any_element(),
        Body::Plain(plain) => div()
            .text_size(TextSize::Sm.font_size())
            .line_height(px(24.))
            .child(plain.clone())
            .into_any_element(),
    };
    let reactions = (!line.reactions.is_empty()).then(|| {
        div().flex().flex_wrap().gap(px(4.)).children(
            line.reactions
                .iter()
                .enumerate()
                .map(|(n, chip)| reaction(ix, n, chip, cx)),
        )
    });
    let files = (!line.files.is_empty()).then(|| {
        div().flex().flex_wrap().gap(px(4.)).children(
            line.files
                .iter()
                .enumerate()
                .map(|(n, file)| file_chip(ix, n, file, cx)),
        )
    });
    let replies = thread.filter(|_| line.replies > 0).map(|open| {
        let root = line.reference.clone();
        let words = if line.replies == 1 {
            "1 reply".to_string()
        } else {
            format!("{} replies", line.replies)
        };
        // The row stretches over the column; the named box is as wide as the button, so a press on its middle is on the button.
        div().flex().child(
            div()
                .debug_selector(move || format!("message-replies-{ix}"))
                .child(
                    Button::new(ElementId::Name(format!("message-replies-{ix}").into()))
                        .label(words)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, window, cx| open(&root, window, cx)),
                ),
        )
    });
    div()
        .debug_selector(move || format!("message-{ix}"))
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(6.))
        .child(
            div()
                .flex_none()
                .pt(px(3.))
                .child(ProjectBadge::new(line.letter.clone(), line.color)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(head)
                .child(text)
                .children(files)
                .children(reactions)
                .children(replies),
        )
        .into_any_element()
}

/// `thumbsup 3`: a small pill, with a wash of the accent when the signed-in person reacted too.
fn reaction(ix: usize, n: usize, chip: &Chip, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let face: SharedString = match emoji_of(&chip.name) {
        Some(emoji) => emoji.into(),
        None => format!(":{}:", chip.name).into(),
    };
    div()
        .debug_selector(move || format!("message-reaction-{ix}-{n}"))
        .flex()
        .items_center()
        .gap(px(4.))
        .h(px(22.))
        .px(px(8.))
        .rounded_full()
        .text_size(TextSize::Xs.font_size())
        .bg(if chip.mine {
            theme.accent.opacity(0.25)
        } else {
            theme.card_strong
        })
        .child(face)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(chip.count.to_string()),
        )
        .into_any_element()
}

/// The emoji a short name stands for, for the few every chat has. Any other name is drawn as `:name:`.
pub(super) fn emoji_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "thumbsup" | "+1" => "👍",
        "thumbsdown" | "-1" => "👎",
        "heart" => "❤️",
        "tada" => "🎉",
        "eyes" => "👀",
        "smile" => "😄",
        "joy" => "😂",
        "fire" => "🔥",
        "rocket" => "🚀",
        "pray" => "🙏",
        "clap" => "👏",
        "white_check_mark" => "✅",
        _ => return None,
    })
}

/// A file: its icon, its name, and its size.
fn file_chip(ix: usize, n: usize, file: &File, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .debug_selector(move || format!("message-file-{ix}-{n}"))
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(10.))
        .rounded(radius::md())
        .bg(theme.card_strong)
        .text_size(TextSize::Xs.font_size())
        .child(
            Icon::new(IconName::AttachFile)
                .size(px(14.))
                .color(theme.muted_foreground),
        )
        .child(file.name.clone())
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(file.detail.clone()),
        )
        .into_any_element()
}
