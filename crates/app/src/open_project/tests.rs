use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
};

use gpui_kit::{IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext, div, px};
use lathe_project::{ChangeSink, Command, Entry, GitOutput, LocalProject, Match, Process, Query};

use super::*;

/// A folder on disk whose watch the test fires itself, on the test's own thread, so every wake is
/// the test scheduler's. The real watch has its own test in lathe-project.
struct Quiet {
    disk: LocalProject,
    sink: Arc<Mutex<Option<ChangeSink>>>,
}

impl Project for Quiet {
    fn root(&self) -> &Path {
        self.disk.root()
    }
    fn list(&self) -> io::Result<Vec<Entry>> {
        self.disk.list()
    }
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.disk.read(path)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.disk.write(path, bytes)
    }
    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(Watch::new(()))
    }
    fn search(&self, query: &Query) -> io::Result<Vec<Match>> {
        self.disk.search(query)
    }
    fn spawn(&self, command: &Command) -> io::Result<Process> {
        self.disk.spawn(command)
    }
    fn git(&self, args: &[&str]) -> io::Result<GitOutput> {
        self.disk.git(args)
    }
}

/// Tells the project `paths` changed, as its watch would.
fn changed(sink: &Arc<Mutex<Option<ChangeSink>>>, paths: &[&str]) {
    let batch = paths.iter().map(|p| Change { path: p.to_string(), kind: ChangeKind::Changed }).collect();
    (sink.lock().unwrap().as_ref().expect("the project watches"))(batch);
}

/// The editor pane around a project, at a fixed size, so the editor lays out as in the app.
struct Pane(Entity<OpenProject>);

impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(700.)).h(px(500.)).child(crate::editor_pane::editor_pane(&self.0, cx))
    }
}

type Opened<'a> = (tempfile::TempDir, Entity<OpenProject>, Arc<Mutex<Option<ChangeSink>>>, &'a mut VisualTestContext);

fn open<'a>(cx: &'a mut TestAppContext, files: &[(&str, &str)]) -> Opened<'a> {
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
    let sink = Arc::new(Mutex::new(None));
    let quiet = Arc::new(Quiet { disk: LocalProject::open(&root).unwrap(), sink: sink.clone() });
    let mut project = None;
    let (_pane, cx) = cx.add_window_view(|window, cx| {
        let p = cx.new(|cx| OpenProject::new(Location::Local { path: root.clone() }, quiet, window, cx));
        project = Some(p.clone());
        Pane(p)
    });
    cx.run_until_parked();
    (dir, project.unwrap(), sink, cx)
}

/// A jump into a file with no tab yet opens it with its line in view, not at the top.
#[gpui_kit::test]
fn a_jump_into_a_new_file_shows_its_line(cx: &mut TestAppContext) {
    let long: String = (0..400).map(|i| format!("line {i}\n")).collect();
    let (dir, project, _, cx) = open(cx, &[("a.txt", "a\n"), ("b.txt", &long)]);
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
    let (_dir, project, _, cx) = open(cx, &[("a.txt", "a\n")]);
    cx.update(|window, cx| {
        project.update(cx, |p, cx| p.jump(Jump { path: Path::new("/usr/lib/rust/lib.rs").into(), position: Position::new(9, 0) }, window, cx));
    });
    cx.run_until_parked();
    assert!(cx.update(|_, cx| project.read(cx).buffers.is_empty()));
}

/// A file changed on disk reloads in a clean tab; a dirty tab keeps its edits and says so.
#[gpui_kit::test]
fn a_change_on_disk_reloads_a_clean_tab_and_asks_in_a_dirty_one(cx: &mut TestAppContext) {
    let (dir, project, sink, cx) = open(cx, &[("clean.txt", "one\n"), ("dirty.txt", "one\n")]);
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
    changed(&sink, &["clean.txt", "dirty.txt"]);
    cx.run_until_parked();
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
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(dir.path().join("dirty.txt")).unwrap(), "mine\n");
}

/// A folder with no git says so, and an empty one lists nothing.
#[gpui_kit::test]
fn an_empty_folder_with_no_git_says_both(cx: &mut TestAppContext) {
    let (_dir, project, _, cx) = open(cx, &[]);
    cx.update(|_, cx| {
        let p = project.read(cx);
        assert_eq!(p.git, Git::None);
        assert!(matches!(&p.listing, Listing::Ready(tree) if tree.is_empty()));
    });
}

/// Closing a tab with unsaved edits asks; Cancel keeps it, Don't Save drops it, and a clean tab
/// closes without asking.
#[gpui_kit::test]
fn closing_a_dirty_tab_asks_first(cx: &mut TestAppContext) {
    let (_dir, project, _, cx) = open(cx, &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    cx.update(|window, cx| {
        project.update(cx, |p, cx| {
            p.open_file("a.txt", window, cx);
            p.open_file("b.txt", window, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| project.update(cx, |p, _| p.buffers.get_mut("a.txt").unwrap().dirty = true));
    cx.update(|window, cx| project.update(cx, |p, cx| p.close_asking("b.txt", window, cx)));
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt(), "a clean tab closes without asking");
    assert!(cx.update(|_, cx| !project.read(cx).buffers.contains_key("b.txt")));
    cx.update(|window, cx| project.update(cx, |p, cx| p.close_asking("a.txt", window, cx)));
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| project.read(cx).buffers.contains_key("a.txt")), "Cancel keeps the tab");
    cx.update(|window, cx| project.update(cx, |p, cx| p.close_asking("a.txt", window, cx)));
    cx.run_until_parked();
    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| project.read(cx).buffers.is_empty()), "Don't Save closes it");
}

/// The watch reporting lathe's own save, after the reader has typed on, is not a change on disk:
/// the file holds what the tab last saved.
#[gpui_kit::test]
fn our_own_save_is_not_a_change_on_disk(cx: &mut TestAppContext) {
    let (_dir, project, sink, cx) = open(cx, &[("a.txt", "one\n")]);
    cx.update(|window, cx| project.update(cx, |p, cx| p.open_file("a.txt", window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let editor = project.read(cx).buffers["a.txt"].editor.clone();
        editor.update(cx, |e, cx| e.set_value("two\n", window, cx));
    });
    cx.update(|_, cx| project.update(cx, |p, cx| p.save(cx)));
    cx.run_until_parked();
    // Typing on after the save, before the watch has said anything.
    cx.update(|_, cx| project.update(cx, |p, _| p.buffers.get_mut("a.txt").unwrap().dirty = true));
    changed(&sink, &["a.txt"]);
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| project.read(cx).buffers["a.txt"].changed_on_disk), "lathe's own write is not news");
}

fn removed(sink: &Arc<Mutex<Option<ChangeSink>>>, path: &str, kind: ChangeKind) {
    (sink.lock().unwrap().as_ref().expect("the project watches"))(vec![Change { path: path.into(), kind }]);
}

fn deleted(project: &Entity<OpenProject>, cx: &mut VisualTestContext, path: &str) -> Deleted {
    cx.update(|_, cx| project.read(cx).buffers[path].deleted)
}

/// A file deleted on disk under its tab says so; Close closes the tab without asking.
#[gpui_kit::test]
fn a_deleted_file_says_so_and_close_closes(cx: &mut TestAppContext) {
    let (dir, project, sink, cx) = open(cx, &[("a.txt", "one\n")]);
    cx.update(|window, cx| project.update(cx, |p, cx| p.open_file("a.txt", window, cx)));
    cx.run_until_parked();
    std::fs::remove_file(dir.path().join("a.txt")).unwrap();
    removed(&sink, "a.txt", ChangeKind::Removed);
    cx.run_until_parked();
    assert_eq!(deleted(&project, cx, "a.txt"), Deleted::Asking);
    cx.update(|_, cx| project.update(cx, |p, cx| p.close("a.txt", cx)));
    assert!(cx.update(|_, cx| project.read(cx).buffers.is_empty()));
    assert!(!cx.has_pending_prompt());
}

/// Keep holds the text as unsaved; a save then asks before it creates the file again.
#[gpui_kit::test]
fn keep_holds_the_text_and_a_save_asks_to_create_the_file(cx: &mut TestAppContext) {
    let (dir, project, sink, cx) = open(cx, &[("a.txt", "one\n")]);
    cx.update(|window, cx| project.update(cx, |p, cx| p.open_file("a.txt", window, cx)));
    cx.run_until_parked();
    std::fs::remove_file(dir.path().join("a.txt")).unwrap();
    removed(&sink, "a.txt", ChangeKind::Removed);
    cx.run_until_parked();
    cx.update(|_, cx| project.update(cx, |p, cx| p.keep_deleted("a.txt", cx)));
    assert_eq!(deleted(&project, cx, "a.txt"), Deleted::Kept);
    assert!(cx.update(|_, cx| project.read(cx).buffers["a.txt"].dirty), "the kept text is unsaved");
    cx.update(|window, cx| project.update(cx, |p, cx| p.save_asking(window, cx)));
    cx.run_until_parked();
    assert!(cx.has_pending_prompt(), "a save asks first");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(!dir.path().join("a.txt").exists(), "Cancel writes nothing");
    cx.update(|window, cx| project.update(cx, |p, cx| p.save_asking(window, cx)));
    cx.run_until_parked();
    cx.simulate_prompt_answer("Create");
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "one\n");
    assert_eq!(deleted(&project, cx, "a.txt"), Deleted::No);
    assert!(!cx.update(|_, cx| project.read(cx).buffers["a.txt"].dirty));
}

/// A file removed and put back at once, as a checkout does, is a change, not a deletion.
#[gpui_kit::test]
fn a_file_put_back_is_no_longer_deleted(cx: &mut TestAppContext) {
    let (dir, project, sink, cx) = open(cx, &[("a.txt", "one\n")]);
    cx.update(|window, cx| project.update(cx, |p, cx| p.open_file("a.txt", window, cx)));
    cx.run_until_parked();
    std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    removed(&sink, "a.txt", ChangeKind::Removed);
    removed(&sink, "a.txt", ChangeKind::Created);
    cx.run_until_parked();
    assert_eq!(deleted(&project, cx, "a.txt"), Deleted::No);
    assert_eq!(cx.update(|_, cx| project.read(cx).buffers["a.txt"].editor.read(cx).value().to_string()), "two\n", "and it reloads");
}
