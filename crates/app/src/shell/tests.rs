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

/// The Settings button holds the window's top right; a press opens the page, whose Back button holds the top
/// left; a press on Back closes it. The sidebar has no foot and the window has no status bar.
#[gpui_kit::test]
fn settings_opens_from_the_top_right_and_back_closes_it(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let gear = cx.debug_bounds("settings-entry").expect("the Settings button is drawn");
    assert!(f32::from(gear.right()) > 1400. - 40. && f32::from(gear.top()) < 40., "at the top right: {gear:?}");
    let layout = cx.debug_bounds("layout-menu").expect("the layout button is drawn");
    assert!(layout.right() <= gear.left(), "the layout button stands left of it: {layout:?} {gear:?}");
    assert!(cx.debug_bounds("settings-back").is_none());
    cx.simulate_click(gear.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(shell.read_with(cx, |s, _| s.settings.is_some()), "a press opens Settings");
    let back = cx.debug_bounds("settings-back").expect("Back is drawn");
    assert!(f32::from(back.left()) < 200. && f32::from(back.top()) < 40., "at the top left: {back:?}");
    assert!(cx.debug_bounds("settings-sections").is_some(), "with its sections");
    cx.simulate_click(back.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(shell.read_with(cx, |s, _| s.settings.is_none()), "Back closes it");
}

/// A notice shows over the foot of the window, and goes by itself.
#[gpui_kit::test]
fn a_notice_floats_and_goes_after_a_few_seconds(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1200.);
    shell.update(cx, |s, cx| s.say("Saved a.txt".into(), cx));
    settle(&shell, cx);
    let notice = cx.debug_bounds("notice").expect("the notice is drawn");
    assert!(f32::from(notice.bottom()) < 900., "it floats inside the window: {notice:?}");
    cx.executor().advance_clock(std::time::Duration::from_secs(6));
    settle(&shell, cx);
    assert!(cx.debug_bounds("notice").is_none(), "and it goes");
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

/// What a press on "Files" in the active project's ⋯ menu sends: the only way into the Files view.
fn open_files_from_the_menu(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext) {
    shell.update_in(cx, |s, window, cx| {
        let project = crate::agents_view::project_id(s.active().unwrap().read(cx));
        let sidebar = s.agents_sidebar.clone();
        s.sidebar_event(&sidebar, &beui::sidebar::SidebarEvent::OpenFiles { project }, window, cx);
    });
    settle(shell, cx);
}

/// Views and commands, part 2: the Sessions view and the Files view, one on screen at a time. The sidebar
/// holds no tree; the Files view holds the tree and the editor, which shows nothing until a file is open.
#[gpui_kit::test]
fn the_project_menu_opens_files_and_a_key_or_button_goes_back(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let sessions = if cfg!(target_os = "macos") { "cmd-1" } else { "ctrl-1" };
    assert!(cx.debug_bounds("sessions-view").is_some(), "the Sessions view shows first");
    assert!(cx.debug_bounds("files-view").is_none() && cx.debug_bounds("files-tree").is_none(), "no tree in it");
    assert!(cx.debug_bounds("view-switch").is_none() && cx.debug_bounds("back-to-sessions").is_none(), "no switch in the Sessions view");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-2" } else { "ctrl-2" });
    settle(&shell, cx);
    assert!(cx.debug_bounds("files-view").is_none(), "⌘2 does nothing: the project menu is the way in");
    open_files_from_the_menu(&shell, cx);
    assert!(cx.debug_bounds("files-view").is_some() && cx.debug_bounds("files-tree").is_some(), "the menu's Files shows the Files view");
    assert!(cx.debug_bounds("back-to-sessions").is_some(), "with a way back");
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

/// Back in the Sessions view the caret is in the composer of the session in front: typing goes on there.
#[gpui_kit::test]
fn going_back_to_sessions_puts_the_caret_in_the_composer(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let sessions = if cfg!(target_os = "macos") { "cmd-1" } else { "ctrl-1" };
    open_files_from_the_menu(&shell, cx);
    cx.simulate_keystrokes(sessions);
    settle(&shell, cx);
    cx.simulate_input("hello");
    settle(&shell, cx);
    let typed = shell.read_with(cx, |s, cx| {
        let project = s.active().cloned().unwrap();
        let session = project.read(cx).sessions[0].clone();
        session.read(cx).composer.read(cx).text(cx).to_string()
    });
    assert_eq!(typed, "hello", "the keys reached the composer");
}

/// While the agent works, its run of tool calls shows in a viewport no taller than `LIVE_HEIGHT`; once the turn
/// ends the run folds to one line of words, and a press on it opens the calls.
#[gpui_kit::test]
fn a_long_run_of_tool_calls_is_capped_while_live_and_folds_after(cx: &mut TestAppContext) {
    use lathe_agents::session::{Event, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus, TurnEnd, TurnOutcome};
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let session = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].clone());
    let tool = |n: usize| {
        [
            Event::ToolStarted(ToolCall {
                id: ToolId::new(format!("t{n}")),
                name: "Bash".into(),
                kind: ToolKind::Shell,
                input: serde_json::json!({ "command": format!("echo {n}") }),
                file: None,
                parent: None,
                status: ToolStatus::Running,
            }),
            Event::ToolFinished { id: ToolId::new(format!("t{n}")), output: ToolOutput { text: format!("{n}"), truncated: false, full_at: None, is_error: false } },
        ]
    };
    session.update(cx, |s, _| {
        s.conversation.user_sent("go");
        for event in (0..10).flat_map(tool) {
            s.conversation.apply(&event);
        }
        s.refresh_rows();
    });
    settle(&shell, cx);
    let live = cx.debug_bounds("activity-live").expect("the run shows as a live group");
    assert!(f32::from(live.size.height) <= crate::activity::LIVE_HEIGHT + 0.5, "the viewport is capped: {:?}", live.size);
    assert!(f32::from(live.size.height) > 150., "and full: {:?}", live.size);
    assert!(cx.debug_bounds("activity-summary").is_none(), "no summary while it works");
    session.update(cx, |s, cx| {
        s.conversation.apply(&Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Completed, summary: None }));
        s.refresh_rows();
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("activity-live").is_none(), "the turn is over: no live viewport");
    let summary = cx.debug_bounds("activity-summary").expect("the run folded to its summary");
    cx.simulate_click(summary.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(session.read_with(cx, |s, _| s.group_is_open(1, 11)), "a press opens the group");
}

/// In a narrow Sessions view the third tab is there only while the right pane holds a review, the pull
/// requests or the tasks: with none of them there is no "Editor" tab, since the editor is the Files view's.
#[gpui_kit::test]
fn a_narrow_sessions_view_has_no_editor_tab(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 700.);
    settle(&shell, cx);
    assert!(cx.debug_bounds("sessions-view").is_some());
    assert!(cx.debug_bounds("narrow-tab-Projects").is_some() && cx.debug_bounds("narrow-tab-Session").is_some());
    assert!(cx.debug_bounds("narrow-tab-Editor").is_none(), "no Editor tab in the Sessions view");
}

/// A panel names the project it works in, and its close button closes it.
#[gpui_kit::test]
fn a_panel_names_its_project_and_a_press_on_its_close_button_closes_it(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    assert!(cx.debug_bounds("panel-project").is_some(), "the panel names its project");
    let name = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].read(cx).project_name());
    assert_eq!(name.as_ref(), dir.path().file_name().unwrap().to_string_lossy());
    let open = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions.len());
    assert_eq!(open(&shell, cx), 1);
    let close = cx.debug_bounds("panel-close").expect("the panel has a close button");
    cx.simulate_click(close.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(open(&shell, cx), 0, "a press closes the panel's session");
    assert!(cx.debug_bounds("panel-close").is_none());
}
