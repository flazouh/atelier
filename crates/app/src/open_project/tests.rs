use std::{path::Path, sync::Arc};

use gpui_kit::{IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext, div, px};
use lathe_project::LocalProject;

use super::*;

/// The editor pane around a project, at a fixed size, so the editor lays out as in the app.
struct Pane(Entity<OpenProject>);

impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(700.)).h(px(500.)).child(crate::editor_pane::editor_pane(&self.0, cx))
    }
}

fn open<'a>(cx: &'a mut TestAppContext, files: &[(&str, &str)]) -> (tempfile::TempDir, Entity<OpenProject>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
        cx.set_reduce_motion(true);
    });
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let at = dir.path().join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    let root = dir.path().to_path_buf();
    let mut project = None;
    let (_pane, cx) = cx.add_window_view(|window, cx| {
        let p = cx.new(|cx| {
            let local = Arc::new(LocalProject::open(&root).unwrap());
            OpenProject::new(Location::Local { path: root.clone() }, local, window, cx)
        });
        project = Some(p.clone());
        Pane(p)
    });
    cx.run_until_parked();
    (dir, project.unwrap(), cx)
}

/// A jump into a file with no tab yet opens it with its line in view, not at the top.
#[gpui_kit::test]
fn a_jump_into_a_new_file_shows_its_line(cx: &mut TestAppContext) {
    let long: String = (0..400).map(|i| format!("line {i}\n")).collect();
    let (dir, project, cx) = open(cx, &[("a.txt", "a\n"), ("b.txt", &long)]);
    let target = dir.path().canonicalize().unwrap().join("b.txt");
    cx.update(|window, cx| {
        project.update(cx, |p, cx| p.jump(Jump { path: target, position: Position::new(300, 0) }, window, cx));
    });
    cx.run_until_parked();
    let (row, scrolled) = cx.update(|_, cx| {
        let buffer = project.read(cx).buffers.get("b.txt").expect("b.txt opened in a tab");
        let editor = buffer.editor.read(cx);
        (editor.cursor_position().line, editor.scroll_offset().y)
    });
    assert_eq!(row, 300, "the caret is on the line");
    assert!(scrolled < px(-4000.), "the view scrolled to it: {scrolled:?}");
}

/// A jump outside the project is named, not opened.
#[gpui_kit::test]
fn a_jump_outside_the_project_is_named(cx: &mut TestAppContext) {
    let (_dir, project, cx) = open(cx, &[("a.txt", "a\n")]);
    cx.update(|window, cx| {
        project.update(cx, |p, cx| p.jump(Jump { path: Path::new("/usr/lib/rust/lib.rs").into(), position: Position::new(9, 0) }, window, cx));
    });
    cx.run_until_parked();
    assert!(cx.update(|_, cx| project.read(cx).buffers.is_empty()));
}
