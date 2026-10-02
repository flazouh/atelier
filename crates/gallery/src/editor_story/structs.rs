use std::path::PathBuf;

use atelier_ui::CodeEditor;
use gpui_kit::{AppContext, Context, Entity, Window, component::input::EditorState};
use atelier_editor::EditorSession;

use crate::{Gallery, workers::workers};
use super::types::FIXTURES;
use super::helpers::write;

/// One tab: a file in a project of its own.
pub(super) struct Fixture {
    pub(super) label: &'static str,
    /// Files that make the directory a project, such as a manifest, as (name, text).
    pub(super) project: &'static [(&'static str, &'static str)],
    /// The file shown, relative to the project.
    pub(super) file: &'static str,
    pub(super) text: &'static str,
}

/// One open tab's editor, and its server once the tab has been shown.
pub(super) struct Tab {
    pub(super) label: &'static str,
    pub(super) path: PathBuf,
    /// The path as the header shows it, relative to its project.
    pub(super) shown: &'static str,
    pub(super) state: Entity<EditorState>,
    pub(super) session: Option<Entity<EditorSession>>,
}

/// Every language's tab, and which one is showing.
pub struct EditorTabs {
    pub(super) tabs: Vec<Tab>,
    pub(super) selected: usize,
}

impl EditorTabs {
    /// Writes each fixture into a project of its own under the temp directory and opens its file.
    pub fn new(window: &mut Window, cx: &mut Context<Gallery>) -> Self {
        let tabs = FIXTURES
            .iter()
            .map(|fixture| {
                let dir = std::env::temp_dir().join(format!("atelier-gallery-{}", fixture.label.to_lowercase()));
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

    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Gallery>) {
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
