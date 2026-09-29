//! The Editor story: a real file per language, each in a small project on disk with its language
//! server. The same keys work in every tab, because nothing below the tab picks a language except the
//! file's own path. A tab's server starts the first time the tab is shown.

use std::path::{Path, PathBuf};

use beui::{ActiveTheme, Badge, Button, ButtonSize, ButtonVariant, CodeEditor, MONO_FONT_FAMILY, TextSize};
use gpui_kit::{
    App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, component::input::EditorState, div, prelude::FluentBuilder, px,
};

use lathe_editor::{EditorSession, file_name, go_to_definition};

use crate::{Gallery, workers::workers};

/// One tab: a file in a project of its own.
struct Fixture {
    label: &'static str,
    /// Files that make the directory a project, such as a manifest, as (name, text).
    project: &'static [(&'static str, &'static str)],
    /// The file shown, relative to the project.
    file: &'static str,
    text: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        label: "Rust",
        project: &[("Cargo.toml", "[package]\nname = \"lathe-gallery\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")],
        file: "src/lib.rs",
        text: SAMPLE_RUST,
    },
    Fixture {
        label: "TypeScript",
        project: &[("tsconfig.json", "{ \"compilerOptions\": { \"strict\": true } }\n")],
        file: "src/shapes.ts",
        text: "// `width` is used twice below: Cmd-click its declaration to list both.\n\nexport function width(): number {\n  return 7;\n}\n\nexport function area(): number {\n  return width() * width();\n}\n\nexport function broken(): number {\n  const text: number = \"not a number\";\n  return text + width();\n}\n",
    },
    Fixture {
        label: "Python",
        project: &[("pyproject.toml", "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n")],
        file: "shapes.py",
        text: "# `width` is used twice below: Cmd-click its declaration to list both.\n\n\ndef width() -> int:\n    return 7\n\n\ndef area() -> int:\n    return width() * width()\n\n\ndef broken() -> int:\n    text: int = \"not a number\"\n    return text + width()\n",
    },
    Fixture {
        label: "Go",
        project: &[("go.mod", "module example.com/shapes\n\ngo 1.21\n")],
        file: "shapes.go",
        text: "// Package shapes: `width` is used twice below; Cmd-click its declaration to list both.\npackage shapes\n\nfunc width() int {\n\treturn 7\n}\n\nfunc area() int {\n\treturn width() * width()\n}\n\nfunc broken() int {\n\tvar text int = \"not a number\"\n\treturn text + width()\n}\n",
    },
    Fixture {
        label: "Java",
        project: &[("pom.xml", "<project><modelVersion>4.0.0</modelVersion><groupId>shapes</groupId><artifactId>shapes</artifactId><version>1</version></project>\n")],
        file: "src/main/java/Shapes.java",
        text: "// `width` is used twice below: Cmd-click its declaration to list both.\npublic class Shapes {\n    static int width() {\n        return 7;\n    }\n\n    static int area() {\n        return width() * width();\n    }\n\n    static int broken() {\n        int text = \"not a number\";\n        return text + width();\n    }\n}\n",
    },
];

/// One open tab's editor, and its server once the tab has been shown.
struct Tab {
    label: &'static str,
    path: PathBuf,
    /// The path as the header shows it, relative to its project.
    shown: &'static str,
    state: Entity<EditorState>,
    session: Option<Entity<EditorSession>>,
}

/// Every language's tab, and which one is showing.
pub struct EditorTabs {
    tabs: Vec<Tab>,
    selected: usize,
}

impl EditorTabs {
    /// Writes each fixture into a project of its own under the temp directory and opens its file.
    pub fn new(window: &mut Window, cx: &mut Context<Gallery>) -> Self {
        let tabs = FIXTURES
            .iter()
            .map(|fixture| {
                let dir = std::env::temp_dir().join(format!("lathe-gallery-{}", fixture.label.to_lowercase()));
                for (name, text) in fixture.project {
                    write(&dir.join(name), text);
                }
                let path = dir.join(fixture.file);
                write(&path, fixture.text);
                Tab {
                    label: fixture.label,
                    state: CodeEditor::state(fixture.file, fixture.text, window, cx),
                    path,
                    shown: fixture.file,
                    session: None,
                }
            })
            .collect();
        Self { tabs, selected: 0 }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Gallery>) {
        self.selected = index;
        self.open(cx);
        cx.notify();
    }

    /// Starts the showing tab's server, once. Call it whenever the story or the tab is shown.
    pub fn open(&mut self, cx: &mut Context<Gallery>) {
        let tab = &mut self.tabs[self.selected];
        if tab.session.is_some() {
            return;
        }
        let (state, path) = (tab.state.clone(), tab.path.clone());
        let session = cx.new(|cx| EditorSession::new(workers(), state, path, cx));
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        tab.session = Some(session);
    }
}

fn write(path: &Path, text: &str) {
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
        move |_: &beui::code_editor::FindReferences, _: &mut Window, cx: &mut App| {
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
                .child(Badge::new(lathe_lsp::language_id(&tab.path).unwrap_or("text")))
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
        .child(div().flex().gap(px(beui::SEGMENT_GAP)).text_size(TextSize::Xs.font_size()).text_color(muted).children(session.read(cx).status()))
        .when(!references.is_empty(), |d| d.child(references_list(&references, &session, cx)))
        .into_any_element()
}

/// The uses of a symbol, one row each: where it is, then its line. A click moves the caret there.
fn references_list(
    references: &[lathe_lsp::Target],
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

/// The Rust tab's file: the one the editor was first built on.
const SAMPLE_RUST: &str = include_str!("editor_story/sample_lib.rs.txt");
