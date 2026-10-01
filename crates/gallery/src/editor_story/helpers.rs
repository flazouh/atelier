use std::path::Path;

use atelier_ui::{
    ActiveTheme, Badge, Button, ButtonSize, ButtonVariant, CodeEditor, MONO_FONT_FAMILY,
    TextSize,
};
use gpui_kit::{
    App, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use atelier_editor::{EditorSession, file_name, go_to_definition};

use crate::Gallery;

pub(super) fn write(path: &Path, text: &str) {
    // The gallery must open on a box with a read-only temp directory; the tab then reports its error.
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

/// The Editor story.
pub fn editor_story(gallery: &Gallery, cx: &mut Context<Gallery>) -> gpui_kit::AnyElement {
    let tabs = &gallery.editors;
    let tab = &tabs.tabs[tabs.selected];
    // `open` starts the session when the story or the tab is shown, a frame before this runs.
    let Some(session) = tab.session.clone() else {
        return div().child("starting the language server").into_any_element();
    };
    let state = tab.state.clone();
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;

    let go = {
        let state = state.clone();
        move |_: &_, window: &mut Window, cx: &mut App| go_to_definition(&state, window, cx)
    };
    let check = {
        let session = session.clone();
        move |_: &_, _: &mut Window, cx: &mut App| session.update(cx, |session, cx| session.check(cx))
    };
    let find = {
        let session = session.clone();
        move |_: &_, _: &mut Window, cx: &mut App| session.update(cx, |session, cx| session.find_references(cx))
    };
    let find_key = {
        let session = session.clone();
        move |_: &atelier_ui::code_editor::FindReferences, _: &mut Window, cx: &mut App| {
            session.update(cx, |session, cx| session.find_references(cx))
        }
    };
    let references = session.read(cx).references().to_vec();

    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .max_w(px(760.))
        .child(div().flex().gap(px(4.)).children(tabs.tabs.iter().enumerate().map(|(index, tab)| {
            let selected = index == tabs.selected;
            Button::new(ElementId::Name(format!("editor-tab-{index}").into()))
                .label(tab.label)
                .size(ButtonSize::Sm)
                .variant(if selected { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                .on_click(cx.listener(move |this, _, _, cx| this.editors.select(index, cx)))
        })))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(TextSize::Xs.font_size())
                .child(div().font_family(MONO_FONT_FAMILY).child(tab.shown))
                .child(Badge::new(atelier_lsp::language_id(&tab.path).unwrap_or("text")))
                .child(div().flex_1())
                .child(Button::new("lsp-check").label("Check").size(ButtonSize::Sm).on_click(check))
                .child(
                    Button::new("lsp-go")
                        .label("Go to definition")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(go),
                )
                .child(
                    Button::new("lsp-refs")
                        .label("Find references")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(find),
                ),
        )
        .child(div().on_action(find_key).child(CodeEditor::new(&state).height(px(380.))))
        .child(div().flex().gap(px(atelier_ui::SEGMENT_GAP)).text_size(TextSize::Xs.font_size()).text_color(muted).children(session.read(cx).status()))
        .when(!references.is_empty(), |d| d.child(references_list(&references, &session, cx)))
        .into_any_element()
}

/// The uses of a symbol, one row each: where it is, then its line. A click moves the caret there.
pub(super) fn references_list(
    references: &[atelier_lsp::Target],
    session: &Entity<EditorSession>,
    cx: &mut Context<Gallery>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let close = {
        let session = session.clone();
        move |_: &_, _: &mut Window, cx: &mut App| session.update(cx, |session, cx| session.close_references(cx))
    };
    let count = references.len();
    div()
        .flex()
        .flex_col()
        .rounded(px(8.))
        .bg(theme.card)
        .p(px(6.))
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .px(px(6.))
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(SharedString::from(format!("{count} uses")))
                .child(div().flex_1())
                .child(Button::new("refs-close").label("Close").variant(ButtonVariant::Ghost).size(ButtonSize::Sm).on_click(close)),
        )
        .children(references.iter().enumerate().map(|(index, target)| {
            let open = {
                let (session, target) = (session.clone(), target.clone());
                move |_: &_, window: &mut Window, cx: &mut App| {
                    session.update(cx, |session, cx| session.open_reference(&target, window, cx))
                }
            };
            div()
                .id(ElementId::Name(format!("reference-{index}").into()))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(6.))
                .py(px(3.))
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|d| d.bg(theme.card_strong))
                .font_family(MONO_FONT_FAMILY)
                .text_size(TextSize::Xs.font_size())
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.muted_foreground)
                        // Line and column, one-based: two uses on one line must not read as one.
                        .child(format!(
                            "{}:{}:{}",
                            file_name(&target.uri),
                            target.range.start.line + 1,
                            target.range.start.character + 1
                        )),
                )
                .child(div().text_color(theme.foreground).child(target.line_text.clone()))
                .on_click(open)
        }))
}
