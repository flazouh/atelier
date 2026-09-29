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

/// Waits, drawing frames, until `done` holds or two seconds pass: a watch batch arrives on its own.
fn until(cx: &mut VisualTestContext, done: impl Fn(&mut VisualTestContext) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !done(cx) && std::time::Instant::now() < deadline {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// A file changed on disk reloads in a clean tab; a dirty tab keeps its edits and says so.
#[gpui_kit::test]
fn a_change_on_disk_reloads_a_clean_tab_and_asks_in_a_dirty_one(cx: &mut TestAppContext) {
    let (dir, project, cx) = open(cx, &[("clean.txt", "one\n"), ("dirty.txt", "one\n")]);
    cx.update(|window, cx| {
        project.update(cx, |p, cx| {
            p.open_file("clean.txt", window, cx);
            p.open_file("dirty.txt", window, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let editor = project.read(cx).buffers["dirty.txt"].editor.clone();
        editor.update(cx, |e, cx| e.set_value("mine\n", window, cx));
    });
    cx.run_until_parked();
    // set_value is not an edit the user typed, so mark it as one would.
    cx.update(|_, cx| project.update(cx, |p, _| p.buffers.get_mut("dirty.txt").unwrap().dirty = true));
    std::fs::write(dir.path().join("clean.txt"), "two\n").unwrap();
    std::fs::write(dir.path().join("dirty.txt"), "two\n").unwrap();
    until(cx, |cx| cx.update(|_, cx| project.read(cx).buffers["dirty.txt"].changed_on_disk));
    until(cx, |cx| cx.update(|_, cx| project.read(cx).buffers["clean.txt"].editor.read(cx).value().as_ref() == "two\n"));
    cx.update(|_, cx| {
        let p = project.read(cx);
        assert_eq!(p.buffers["clean.txt"].editor.read(cx).value().as_ref(), "two\n", "the clean tab reloaded");
        assert!(!p.buffers["clean.txt"].dirty);
        assert!(p.buffers["dirty.txt"].changed_on_disk, "the dirty tab says the file changed");
        assert_eq!(p.buffers["dirty.txt"].editor.read(cx).value().as_ref(), "mine\n", "and keeps its edits");
    });
    // Keep mine, then save: the tab's text wins on disk.
    cx.update(|_, cx| project.update(cx, |p, cx| p.keep_mine("dirty.txt", cx)));
    cx.update(|window, cx| project.update(cx, |p, cx| p.open_file("dirty.txt", window, cx)));
    cx.update(|_, cx| project.update(cx, |p, cx| p.save(cx)));
    until(cx, |_| std::fs::read_to_string(dir.path().join("dirty.txt")).unwrap() == "mine\n");
    assert_eq!(std::fs::read_to_string(dir.path().join("dirty.txt")).unwrap(), "mine\n");
}

/// A folder with no git says so, and an empty one lists nothing.
#[gpui_kit::test]
fn an_empty_folder_with_no_git_says_both(cx: &mut TestAppContext) {
    let (_dir, project, cx) = open(cx, &[]);
    until(cx, |cx| cx.update(|_, cx| project.read(cx).git != Git::Unknown && !matches!(project.read(cx).listing, Listing::Loading)));
    cx.update(|_, cx| {
        let p = project.read(cx);
        assert_eq!(p.git, Git::None);
        assert!(matches!(&p.listing, Listing::Ready(tree) if tree.is_empty()));
    });
}
