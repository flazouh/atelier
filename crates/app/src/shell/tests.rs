use gpui_kit::{TestAppContext, px, size};

use super::*;

#[gpui_kit::test]
fn the_first_launch_shows_the_mark_and_one_line_about_what_lathe_is_above_the_buttons(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&lathe_settings::Settings::default(), cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    for _ in 0..3 {
        cx.run_until_parked();
        shell.update(cx, |_, cx| cx.notify());
    }
    let mark = cx.debug_bounds("lathe-mark").expect("the mark is drawn");
    let line = cx.debug_bounds("first-launch-line").expect("the line is drawn");
    let button = cx.debug_bounds("open-folder").or_else(|| cx.debug_bounds("open-folder-button"));
    assert_eq!((f32::from(mark.size.width), f32::from(mark.size.height)), (40., 40.));
    assert!(mark.bottom() <= line.top(), "the mark is above the line: {mark:?} {line:?}");
    if let Some(button) = button {
        assert!(line.bottom() <= button.top(), "the line is above the buttons");
    }
    assert!(WHAT_LATHE_IS.split_whitespace().count() <= 20, "one short line");
}
/// Quit has a key, Command-Q on the Mac and Control-Q elsewhere.
#[gpui_kit::test]
fn quit_has_its_key(cx: &mut TestAppContext) {
    cx.update(|cx| {
        bind_keys(cx);
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let bound: Vec<String> = keymap.bindings_for_action(&Quit).map(|b| b.keystrokes().iter().map(|k| k.unparse()).collect::<Vec<_>>().join(" ")).collect();
        let want = if cfg!(target_os = "macos") { "cmd-q" } else { "ctrl-q" };
        assert!(bound.iter().any(|b| b == want), "{bound:?}");
    });
}

fn open_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut gpui_kit::VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&lathe_settings::Settings::default(), cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    (shell, cx)
}

fn settle(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        shell.update(cx, |_, cx| cx.notify());
    }
}

/// Start screen: a long error shows in the screen above Recent and wraps inside its column.
#[gpui_kit::test]
fn a_long_error_shows_on_the_start_screen_above_recent_and_wraps(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    shell.update(cx, |s, cx| s.say("Could not open the folder: the path does not exist and this line is long enough to need more than one line of the start screen column".into(), cx));
    settle(&shell, cx);
    let error = cx.debug_bounds("start-error").expect("the error is on the start screen");
    let recent = cx.debug_bounds("recent-heading").expect("Recent is drawn");
    assert!(error.bottom() <= recent.top(), "the error is above Recent");
    assert!(error.size.width <= px(420.), "the error stays in the column: {error:?}");
    assert!(error.size.height > px(20.), "the error wraps to more than one line: {error:?}");
}

/// A view that draws only the sidebar foot.
struct Foot(Entity<Shell>);

impl Render for Foot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let foot = self.0.update(cx, |shell, cx| shell.sidebar_foot(cx));
        div().w(px(240.)).child(foot)
    }
}

/// A12: the sidebar foot holds the Settings entry, with its key on the cap, and no theme picker.
#[gpui_kit::test]
fn the_sidebar_foot_holds_settings_and_not_the_theme_picker(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let (foot, cx) = cx.add_window_view(|_, _| Foot(shell.clone()));
    cx.simulate_resize(size(px(300.), px(200.)));
    for _ in 0..3 {
        cx.run_until_parked();
        foot.update(cx, |_, cx| cx.notify());
    }
    assert!(cx.debug_bounds("settings-entry").is_some(), "the Settings entry stands in the foot");
    assert!(cx.debug_bounds("theme").is_none(), "no theme picker in the foot");
}

/// K1: every global chord reaches its action from each place the focus can be. Tasks is the chord that
/// was lost; each pane is given the focus in turn, and the chord must show Tasks from there.
#[gpui_kit::test]
fn the_tasks_chord_reaches_its_action_from_each_pane(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    let chord = if cfg!(target_os = "macos") { "cmd-shift-l" } else { "ctrl-shift-l" };
    let front = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| {
        shell.read_with(cx, |s, cx| s.active().map(|p| p.read(cx).front()))
    };
    type Place = fn(&mut Shell, &mut Window, &mut Context<Shell>);
    let places: [(&str, Place); 4] = [
        ("nothing", |_, window, cx| window.blur(cx)),
        ("the shell", |s, window, cx| s.focus.focus(window, cx)),
        ("the sidebar", |s, window, cx| s.agents_sidebar.read(cx).focus_handle(cx).focus(window, cx)),
        ("a session's composer", |s, window, cx| s.new_session_key(&NewSession, window, cx)),
    ];
    let tasks = Some(crate::open_project::front::Front::Tasks);
    let mut lost = Vec::new();
    for (name, place) in places {
        shell.update_in(cx, place);
        settle(&shell, cx);
        assert_ne!(front(&shell, cx), tasks, "Tasks starts hidden ({name})");
        cx.simulate_keystrokes(chord);
        settle(&shell, cx);
        if front(&shell, cx) != tasks {
            lost.push(format!("from {name}"));
            continue;
        }
        // From the Tasks pane itself the chord hides it again.
        cx.simulate_keystrokes(chord);
        settle(&shell, cx);
        if front(&shell, cx) == tasks {
            lost.push(format!("from the Tasks pane, after {name}"));
            shell.update_in(cx, |s, window, cx| s.show_tasks(window, cx));
        }
    }
    assert!(lost.is_empty(), "{chord} did not reach Tasks {lost:?}");
}

/// K1, for the whole table: every lathe chord bound with no context is on the dispatch path from each place
/// the focus can be, so no pane and no lost focus eats it.
#[gpui_kit::test]
fn every_global_chord_reaches_its_action_from_each_pane(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    let global: Vec<(String, Box<dyn gpui_kit::Action>)> = cx.update(|_, cx| {
        let map = cx.key_bindings();
        let map = map.borrow();
        map.bindings()
            .filter(|b| b.predicate().is_none() && b.action().name().starts_with("lathe::"))
            .map(|b| (b.keystrokes().iter().map(|k| k.inner().unparse()).collect::<Vec<_>>().join(" "), b.action().boxed_clone()))
            .collect()
    });
    assert!(global.len() >= 10, "the shell's chords are bound: {}", global.len());
    type Place = fn(&mut Shell, &mut Window, &mut Context<Shell>);
    let places: [(&str, Place); 5] = [
        ("nothing", |_, window, cx| window.blur(cx)),
        ("the shell", |s, window, cx| s.focus.focus(window, cx)),
        ("the sidebar", |s, window, cx| s.agents_sidebar.read(cx).focus_handle(cx).focus(window, cx)),
        ("a session's composer", |s, window, cx| s.new_session_key(&NewSession, window, cx)),
        ("the Tasks pane", |s, window, cx| s.show_tasks(window, cx)),
    ];
    let mut lost = Vec::new();
    for (name, place) in places {
        shell.update_in(cx, place);
        settle(&shell, cx);
        let missing: Vec<String> = cx.update(|window, cx| {
            global.iter().filter(|(_, action)| !window.is_action_available(action.as_ref(), cx)).map(|(keys, _)| keys.clone()).collect()
        });
        if !missing.is_empty() {
            lost.push(format!("from {name}: {missing:?}"));
        }
    }
    assert!(lost.is_empty(), "chords that do not reach their action {lost:#?}");
}

/// View cache: the sidebar is drawn from its last frame until it changes. A renamed session must still show
/// its new title in the next frame.
#[gpui_kit::test]
fn a_cached_sidebar_shows_a_renamed_session(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.new_session_key(&NewSession, window, cx));
    settle(&shell, cx);
    let session = shell.read_with(cx, |s, cx| s.active().unwrap().read(cx).sessions[0].clone());
    let before = "New session";
    assert!(cx.debug_bounds("row-title:New session").is_some(), "the row shows {before:?}");
    session.update(cx, |s, cx| {
        s.name = Some("Renamed by the reader".into());
        cx.emit(crate::agent_session::SessionEvent::Renamed);
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("row-title:Renamed by the reader").is_some(), "the cached sidebar still shows {before:?}");
}

/// View cache: a session's age ("2m") comes from the time of the last sync. The cached sidebar is given the
/// time again each minute while it shows a session, so an age never stands still.
#[gpui_kit::test]
fn a_cached_sidebar_is_given_the_time_each_minute(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.new_session_key(&NewSession, window, cx));
    settle(&shell, cx);
    let sidebar = shell.read_with(cx, |s, _| s.agents_sidebar.clone());
    let told = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = told.clone();
    let _watch = cx.update(|_, cx| cx.observe(&sidebar, move |_, _| count.set(count.get() + 1)));
    cx.executor().advance_clock(std::time::Duration::from_secs(61));
    cx.run_until_parked();
    assert!(told.get() >= 1, "no new time reached the sidebar in a minute");
}

/// View cache: the right pane is drawn from its last frame until its project changes. Changed at the project
/// itself, not through the shell (which moves the focus and so draws everything again), the next frame shows
/// the new front: Tasks, and then not. (The editor is the Files view's.)
#[gpui_kit::test]
fn a_cached_right_pane_follows_its_project(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1600.), px(900.)));
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    assert!(cx.debug_bounds("tasks-mode-list").is_none(), "Tasks starts hidden");
    cx.update(|window, cx| project.update(cx, |p, cx| p.toggle_tasks(window, cx)));
    settle(&shell, cx);
    assert!(cx.debug_bounds("tasks-mode-list").is_some(), "the cached right pane still shows the editor");
    cx.update(|window, cx| project.update(cx, |p, cx| p.toggle_tasks(window, cx)));
    settle(&shell, cx);
    assert!(cx.debug_bounds("tasks-mode-list").is_none(), "the cached right pane still shows Tasks");
}

/// View cache: each session's panel is drawn from its last frame until its session changes. A renamed
/// session's panel header shows the new title in the next frame.
#[gpui_kit::test]
fn a_cached_panel_follows_its_session(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1600.), px(900.)));
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.new_session_key(&NewSession, window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("panel-title:New session").is_some(), "the panel shows its title");
    let session = shell.read_with(cx, |s, cx| s.active().unwrap().read(cx).sessions[0].clone());
    session.update(cx, |s, cx| {
        s.name = Some("Renamed by the reader".into());
        cx.emit(crate::agent_session::SessionEvent::Renamed);
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("panel-title:Renamed by the reader").is_some(), "the cached panel still shows the old title");
}

/// Views and commands, part 1: no panels bar. A ⋯ at the top right of the session area opens the layout
/// menu: "Side by side" and "Single view" as a choice of one, "Group by project" as a check, each with its
/// key; choosing one changes the panels.
#[gpui_kit::test]
fn the_layout_lives_in_a_menu_and_the_bar_is_gone(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1600.), px(900.)));
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.new_session_key(&NewSession, window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("layout-switch").is_none(), "the bar's layout switch is gone");
    assert!(cx.debug_bounds("grouping-switch").is_none(), "and its grouping switch");
    let button = cx.debug_bounds("layout-menu").expect("the ⋯ layout button is drawn");
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    for row in ["layout-side-by-side", "layout-single", "layout-grouped"] {
        assert!(cx.debug_bounds(row).is_some(), "the menu has {row}");
    }
    let single = cx.debug_bounds("layout-single").unwrap();
    cx.simulate_click(single.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    let layout = shell.read_with(cx, |s, cx| s.panels.read(cx).layout());
    assert_eq!(layout, beui::panel_types::Layout::Single, "Single view is chosen");
}

/// A shell with a project and one session, at `width`.
fn with_a_session(cx: &mut TestAppContext, width: f32) -> (Entity<Shell>, &mut gpui_kit::VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(width), px(900.)));
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.new_session_key(&NewSession, window, cx));
    settle(&shell, cx);
    (shell, cx, dir)
}

/// Views and commands, part 2: the Sessions view and the Files view, one on screen at a time. The sidebar
/// holds no tree; the Files view holds the tree and the editor, which shows nothing until a file is open.
#[gpui_kit::test]
fn one_view_shows_at_a_time_and_the_keys_switch_them(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let files = if cfg!(target_os = "macos") { "cmd-2" } else { "ctrl-2" };
    let sessions = if cfg!(target_os = "macos") { "cmd-1" } else { "ctrl-1" };
    assert!(cx.debug_bounds("sessions-view").is_some(), "the Sessions view shows first");
    assert!(cx.debug_bounds("files-view").is_none() && cx.debug_bounds("files-tree").is_none(), "no tree in it");
    cx.simulate_keystrokes(files);
    settle(&shell, cx);
    assert!(cx.debug_bounds("files-view").is_some() && cx.debug_bounds("files-tree").is_some(), "{files} shows the Files view");
    assert!(cx.debug_bounds("sessions-view").is_none(), "and only it");
    assert!(cx.debug_bounds("editor-tab-0").is_none(), "no editor before a file is open");
    cx.simulate_keystrokes(sessions);
    settle(&shell, cx);
    assert!(cx.debug_bounds("sessions-view").is_some() && cx.debug_bounds("files-view").is_none(), "{sessions} goes back");
}

/// Opening a file from a session (a row's file name) goes to the Files view with the file open.
#[gpui_kit::test]
fn opening_a_file_from_a_session_goes_to_the_files_view(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    project.update(cx, |_, cx| cx.emit(crate::open_project::ProjectEvent::Open("a.txt".into())));
    settle(&shell, cx);
    assert!(cx.debug_bounds("files-view").is_some(), "the Files view shows");
    assert!(cx.debug_bounds("editor-tab-0").is_some(), "with a.txt open");
}

/// In a narrow window the Files view shows the tree or the editor, one at a time, with a tab for each.
#[gpui_kit::test]
fn a_narrow_files_view_has_a_tab_for_the_tree_and_the_editor(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 700.);
    shell.update_in(cx, |s, window, cx| s.show_view(ShellView::Files, window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("narrow-tab-Files").is_some() && cx.debug_bounds("narrow-tab-Editor").is_some(), "Files and Editor tabs");
    assert!(cx.debug_bounds("files-tree").is_some(), "the tree shows first");
}

/// The view in front is kept: a window opens on the view it closed on.
#[gpui_kit::test]
fn a_window_opens_on_the_view_it_closed_on(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
    });
    let saved = lathe_settings::Settings { view: Some("files".into()), ..Default::default() };
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&saved, cx));
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Files);
    assert_eq!(ShellView::from_words(Some("sessions")), ShellView::Sessions);
    assert_eq!(ShellView::from_words(None), ShellView::Sessions, "a first launch opens on Sessions");
    assert_eq!(ShellView::Files.words(), "files");
}

/// In a narrow Files view, a file picked in the tree opens in the editor, which comes to the front.
#[gpui_kit::test]
fn a_file_picked_in_a_narrow_tree_shows_in_the_editor(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 700.);
    shell.update_in(cx, |s, window, cx| s.show_view(ShellView::Files, window, cx));
    settle(&shell, cx);
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("editor-tab-0").is_some(), "the editor shows a.txt");
}
