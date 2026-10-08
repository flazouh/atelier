use gpui_kit::Focusable;
use gpui_kit::{TestAppContext, px, size};

use super::*;

#[gpui_kit::test]
fn the_first_launch_shows_the_mark_and_one_line_about_what_atelier_is_above_the_buttons(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&atelier_settings::Settings::default(), cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    for _ in 0..3 {
        cx.run_until_parked();
        shell.update(cx, |_, cx| cx.notify());
    }
    let mark = cx.debug_bounds("atelier-mark").expect("the mark is drawn");
    let line = cx.debug_bounds("first-launch-line").expect("the line is drawn");
    let button = cx.debug_bounds("open-folder").or_else(|| cx.debug_bounds("open-folder-button"));
    assert_eq!((f32::from(mark.size.width), f32::from(mark.size.height)), (40., 40.));
    assert!(mark.bottom() <= line.top(), "the mark is above the line: {mark:?} {line:?}");
    if let Some(button) = button {
        assert!(line.bottom() <= button.top(), "the line is above the buttons");
    }
    assert!(WHAT_ATELIER_IS.split_whitespace().count() <= 20, "one short line");
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
    crate::open_project::TEST_THREAD_ONLY.set(true);
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&atelier_settings::Settings::default(), cx));
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

/// A line the start screen shows, such as a folder that is not there, stays until a project opens.
#[gpui_kit::test]
fn a_start_screen_error_stays_until_a_project_opens(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    shell.update(cx, |s, cx| s.say("Could not open the folder: no such folder".into(), cx));
    cx.executor().advance_clock(std::time::Duration::from_secs(6));
    settle(&shell, cx);
    assert!(cx.debug_bounds("start-error").is_some(), "it is still there after the notice time");
    let dir = tempfile::tempdir().unwrap();
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("start-error").is_none(), "and it goes when a project opens");
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
    assert!(cx.debug_bounds("layout-menu").is_none(), "the layout button is not on the Settings page");
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
    let front = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| shell.read_with(cx, |s, _| s.view);
    type Place = fn(&mut Shell, &mut Window, &mut Context<Shell>);
    let places: [(&str, Place); 4] = [
        ("nothing", |_, window, cx| window.blur(cx)),
        ("the shell", |s, window, cx| s.focus.focus(window, cx)),
        ("the sidebar", |s, window, cx| s.agents_sidebar.read(cx).focus_handle(cx).focus(window, cx)),
        ("a session's composer", |s, window, cx| s.new_session_key(&NewSession, window, cx)),
    ];
    let tasks = ShellView::Tasks;
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

/// K1, for the whole table: every atelier chord bound with no context is on the dispatch path from each place
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
            .filter(|b| b.predicate().is_none() && b.action().name().starts_with("atelier::"))
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
    assert_eq!(layout, atelier_ui::panel_types::Layout::Single, "Single view is chosen");
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
        s.sidebar_event(&sidebar, &atelier_ui::sidebar::SidebarEvent::OpenFiles { project }, window, cx);
    });
    settle(shell, cx);
}

/// The project offers two agents; the second is the target of a handoff.
fn offering_two_agents(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext) {
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    project.update(cx, |p, cx| {
        p.offer(vec![crate::fake_agent::named_agent("alpha"), crate::fake_agent::named_agent("beta")], cx);
    });
    settle(shell, cx);
}

fn the_second_session(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext) -> Option<(&'static str, bool)> {
    shell.read_with(cx, |s, cx| {
        let next = s.active().unwrap().read(cx).sessions.get(1)?.read(cx);
        Some((next.agent.name, next.continues().is_some()))
    })
}

/// "Handoff" in a session's menu opens a new session in its project, on the agent it names, that carries the old one on, in front.
#[gpui_kit::test]
fn a_handoff_from_the_sidebar_opens_a_session_on_the_chosen_agent_that_carries_the_old_one_on(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    offering_two_agents(&shell, cx);
    let (project, first) = shell.read_with(cx, |s, cx| {
        let p = s.active().unwrap().read(cx);
        (crate::agents_view::project_id(p), p.sessions[0].read(cx).key.clone())
    });

    shell.update_in(cx, |s, window, cx| {
        let sidebar = s.agents_sidebar.clone();
        s.sidebar_event(&sidebar, &atelier_ui::sidebar::SidebarEvent::Handoff { project, session: first.clone(), target: "beta".into() }, window, cx);
    });
    settle(&shell, cx);

    assert_eq!(the_second_session(&shell, cx), Some(("beta", true)), "on beta, continuing the first");
    assert!(cx.debug_bounds("session-heading").is_some());
}

/// A target the project does not offer opens nothing.
#[gpui_kit::test]
fn a_handoff_to_an_agent_the_project_does_not_offer_opens_nothing(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    offering_two_agents(&shell, cx);
    let (project, first) = shell.read_with(cx, |s, cx| {
        let p = s.active().unwrap().read(cx);
        (crate::agents_view::project_id(p), p.sessions[0].read(cx).key.clone())
    });

    shell.update_in(cx, |s, window, cx| {
        let sidebar = s.agents_sidebar.clone();
        s.sidebar_event(&sidebar, &atelier_ui::sidebar::SidebarEvent::Handoff { project, session: first, target: "gamma".into() }, window, cx);
    });
    settle(&shell, cx);

    assert_eq!(the_second_session(&shell, cx), None);
}

/// An account at its usage limit says so over the composer, and its Handoff menu carries the session on in a new one,
/// on the agent the reader chooses.
#[gpui_kit::test]
fn a_reached_limit_offers_a_handoff_menu_of_the_agents(cx: &mut TestAppContext) {
    use atelier_agents::session::{Event, Limit, LimitState, LimitWindow};
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    offering_two_agents(&shell, cx);
    assert!(cx.debug_bounds("limit-notice").is_none(), "no box while there is room");
    let first = shell.read_with(cx, |s, cx| s.active().unwrap().read(cx).sessions[0].clone());
    first.update(cx, |s, cx| {
        s.conversation.apply(&Event::Limit(Limit { state: LimitState::Reached, resets_at: None, window: Some(LimitWindow::FiveHour) }));
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("limit-notice").is_some(), "the box shows");

    let button = cx.debug_bounds("limit-handoff").expect("with its button");
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("menu-lead-beta").is_some(), "each agent leads with its mark or monogram");
    let beta = cx.debug_bounds("branch-beta").expect("the button opens the agents");
    cx.simulate_click(beta.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);

    assert_eq!(the_second_session(&shell, cx), Some(("beta", true)));
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
    assert!(cx.debug_bounds("code-sidebar").is_some() && cx.debug_bounds("rail-sessions").is_some(), "in the Code lens, with the rail the way back");
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
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
    });
    let saved = atelier_settings::Settings { view: Some("files".into()), ..Default::default() };
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

/// A tree row fills the list: its hover and its selection reach from one margin to the other, whatever the name.
#[gpui_kit::test]
fn a_tree_row_takes_the_full_width_of_the_tree(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    settle(&shell, cx);
    let tree = cx.debug_bounds("files-tree").expect("the tree shows");
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    let (tree, row) = (f32::from(tree.size.width), f32::from(row.size.width));
    assert!((tree - row - 12.).abs() < 1., "the row ({row}) fills the tree ({tree}) less its two 6 px margins");
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
    use atelier_agents::session::{Event, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus, TurnEnd, TurnOutcome};
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
    let badge = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].read(cx).badge.clone());
    let sidebar = shell.read_with(cx, |s, cx| s.agents_sidebar.read(cx).all_projects()[0].badge.clone());
    assert_eq!(badge, Some(sidebar), "the panel wears the badge the sidebar draws for the project");
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

/// Projects are added from the button joined to the sidebar head's switcher; the sidebar's head keeps only the filter behind its ⋯.
#[gpui_kit::test]
fn the_switcher_adds_projects_and_the_sidebar_head_filters_sessions(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    assert!(cx.debug_bounds("session-filter").is_none(), "no box that narrows the sessions by title");
    let add = cx.debug_bounds("add-project").expect("the add button is drawn");
    let switcher = cx.debug_bounds("project-switcher").unwrap();
    assert!((add.left() - switcher.right()).abs() < gpui_kit::px(2.) && add.top() < switcher.bottom(), "joined to the switcher: {add:?} after {switcher:?}");
    cx.simulate_click(add.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("add-folder").is_some() && cx.debug_bounds("add-ssh").is_some(), "the add menu offers a folder and SSH");
    cx.simulate_keystrokes("escape");
    settle(&shell, cx);
    let options = cx.debug_bounds("sidebar-options").expect("the ⋯ is drawn");
    cx.simulate_click(options.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    for choice in atelier_ui::sidebar_filter::SessionFilter::ALL {
        assert!(cx.debug_bounds(choice.row()).is_some(), "{} is on the menu", choice.words());
    }
    let archived = cx.debug_bounds("filter-archived").unwrap();
    cx.simulate_click(archived.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.agents_sidebar.read(cx).layout().filter), atelier_ui::sidebar_filter::SessionFilter::Archived);
}

/// An archived session leaves the list until the Archived or All filter asks for it, and comes back when taken out.
#[gpui_kit::test]
fn an_archived_session_leaves_the_list_until_the_filter_asks(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    project.update(cx, |p, _| p.past = vec![atelier_agents::session::SessionSummary { id: atelier_agents::session::SessionId::new("old-1"), title: "an old idea".into(), updated: Some(5), account: None }]);
    shell.update(cx, |s, cx| s.sync(cx));
    let titles = |cx: &mut gpui_kit::VisualTestContext| {
        shell.read_with(cx, |s, cx| s.agents_sidebar.read(cx).projects().iter().flat_map(|p| p.sessions.iter().map(|x| x.title.to_string())).collect::<Vec<_>>())
    };
    assert!(titles(cx).iter().any(|t| t == "an old idea"), "a past session is on the list");
    shell.update(cx, |s, cx| s.set_archived("past:old-1", true, cx));
    assert!(!titles(cx).iter().any(|t| t == "an old idea"), "archived: gone from the list");
    shell.update(cx, |s, cx| s.agents_sidebar.update(cx, |sb, cx| sb.choose_filter(atelier_ui::sidebar_filter::SessionFilter::Archived, cx)));
    assert_eq!(titles(cx), ["an old idea"], "the Archived filter shows only it");
    shell.update(cx, |s, cx| s.set_archived("past:old-1", false, cx));
    shell.update(cx, |s, cx| s.agents_sidebar.update(cx, |sb, cx| sb.choose_filter(atelier_ui::sidebar_filter::SessionFilter::Active, cx)));
    assert!(titles(cx).iter().any(|t| t == "an old idea"), "taken out of the archive: back on the list");
}

/// The panel's head has no status words; its ⋯ menu holds what a reader does with a session, and a choice does it.
#[gpui_kit::test]
fn the_panel_menu_holds_the_actions_for_a_session(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    assert!(cx.debug_bounds("panel-review").is_none(), "nothing to review before a turn changed something");
    let more = cx.debug_bounds("panel-more").expect("the panel has a ⋯ button");
    cx.simulate_click(more.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    for row in ["panel-rename", "panel-new-session", "panel-files", "panel-archive"] {
        assert!(cx.debug_bounds(row).is_some(), "the menu has {row}");
    }
    for words in ["Rename", "New session in this project", "Open the project's files", "Archive"] {
        let icon: &'static str = Box::leak(format!("menu-icon-{words}").into_boxed_str());
        assert!(cx.debug_bounds(icon).is_some(), "{words} has an icon");
    }
    let open = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions.len());
    assert_eq!(open(&shell, cx), 1);
    let fresh = cx.debug_bounds("panel-new-session").unwrap();
    cx.simulate_click(fresh.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(open(&shell, cx), 2, "New session in this project opens another panel");
    assert!(cx.debug_bounds("panel-rename").is_none(), "and the menu shut");
}

/// Only the single view has a session in front, so only there does the sidebar mark one.
#[gpui_kit::test]
fn the_sidebar_marks_the_open_session_in_the_single_view_only(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let marked = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| shell.read_with(cx, |s, cx| s.agents_sidebar.read(cx).open_session().is_some());
    assert!(!marked(&shell, cx), "side by side: no row is marked");
    shell.update(cx, |s, cx| s.choose_layout(atelier_ui::panel_types::Layout::Single, cx));
    settle(&shell, cx);
    assert!(marked(&shell, cx), "single view: the one in front is marked");
    shell.update(cx, |s, cx| s.choose_layout(atelier_ui::panel_types::Layout::SideBySide, cx));
    settle(&shell, cx);
    assert!(!marked(&shell, cx), "back side by side: none");
}

/// In the single view the session tabs stand in the title bar, not above the panel; side by side there are none.
#[gpui_kit::test]
fn the_single_views_tabs_stand_in_the_title_bar(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    assert!(cx.debug_bounds("title-tabs").is_none(), "side by side: the title bar has no tabs");
    shell.update(cx, |s, cx| s.choose_layout(atelier_ui::panel_types::Layout::Single, cx));
    settle(&shell, cx);
    let tabs = cx.debug_bounds("title-tabs").expect("the tabs have a zone in the title bar");
    assert!(tabs.bottom() <= gpui_kit::px(super::types::TITLE_BAR), "the zone is inside the title bar: {tabs:?}");
    assert!(shell.read_with(cx, |s, cx| s.panels.read(cx).tabs_hoisted()), "the panels leave their own bar out");
    shell.update(cx, |s, cx| s.choose_layout(atelier_ui::panel_types::Layout::SideBySide, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("title-tabs").is_none(), "back side by side: none");
    assert!(!shell.read_with(cx, |s, cx| s.panels.read(cx).tabs_hoisted()), "and the panels are as they were");
}

/// The Files view's file tabs stand in the title bar too.
#[gpui_kit::test]
fn the_files_views_tabs_stand_in_the_title_bar(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    let tab = cx.debug_bounds("editor-tab-0").expect("a.txt has a tab");
    assert!(tab.bottom() <= gpui_kit::px(super::types::TITLE_BAR), "the tab is in the title bar: {tab:?}");
}

/// A name typed in the tree makes a file, renames one and copies one on the disk, and the tree lists the result.
#[gpui_kit::test]
fn the_tree_makes_renames_and_copies_files_on_the_disk(cx: &mut TestAppContext) {
    use crate::open_project::tree_edit::TreeEditKind;
    let (shell, cx, dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    let type_name = |kind: TreeEditKind, name: &str, cx: &mut gpui_kit::VisualTestContext| {
        project.update_in(cx, |p, window, cx| p.start_tree_edit(kind, window, cx));
        settle(&shell, cx);
        assert!(cx.debug_bounds("tree-edit-row").is_some(), "the name has a row in the tree");
        let input = project.read_with(cx, |p, _| p.tree_edit.as_ref().unwrap().input.clone());
        input.update_in(cx, |i, window, cx| i.set_value(name, window, cx));
        project.update(cx, |p, cx| p.commit_tree_edit(cx));
        settle(&shell, cx);
    };
    type_name(TreeEditKind::NewFile { parent: String::new() }, "b.txt", cx);
    assert!(dir.path().join("b.txt").exists(), "a new file is made");
    type_name(TreeEditKind::NewFolder { parent: String::new() }, "docs", cx);
    assert!(dir.path().join("docs").is_dir(), "a new folder is made");
    type_name(TreeEditKind::Rename { path: "a.txt".into() }, "c.txt", cx);
    assert!(!dir.path().join("a.txt").exists() && dir.path().join("c.txt").exists(), "a file is renamed");
    assert!(cx.debug_bounds("tree-row-c.txt").is_some(), "and the tree lists it under its new name");
    project.update(cx, |p, cx| p.duplicate("c.txt", cx));
    settle(&shell, cx);
    assert_eq!(std::fs::read_to_string(dir.path().join("c copy.txt")).unwrap(), "a", "a copy beside it");
    // A name that is there already is refused, and the typing stays.
    project.update_in(cx, |p, window, cx| p.start_tree_edit(TreeEditKind::NewFile { parent: String::new() }, window, cx));
    let input = project.read_with(cx, |p, _| p.tree_edit.as_ref().unwrap().input.clone());
    input.update_in(cx, |i, window, cx| i.set_value("b.txt", window, cx));
    project.update(cx, |p, cx| p.commit_tree_edit(cx));
    settle(&shell, cx);
    assert!(project.read_with(cx, |p, _| p.tree_edit.is_some()), "the name is kept to change");
}

/// A mention from the tree waits as a chip in the composer of the session chosen.
#[gpui_kit::test]
fn a_file_mentioned_from_the_tree_is_a_chip_in_the_session(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let (project, session) = shell.read_with(cx, |s, cx| {
        let project = s.active().cloned().unwrap();
        let session = project.read(cx).sessions[0].clone();
        (project, session)
    });
    project.update(cx, |p, cx| p.mention_in(&session, "src/a.rs", cx));
    settle(&shell, cx);
    let chips = cx.update(|_, cx| session.read(cx).composer.read(cx).chips().iter().map(|c| (c.label.to_string(), c.mention.clone().map(|m| m.to_string()))).collect::<Vec<_>>());
    assert_eq!(chips, [("a.rs".to_string(), Some("@src/a.rs".to_string()))]);
}

/// The right press on a row opens the menu with what a file offers, and the empty part of the tree opens a shorter one.
#[gpui_kit::test]
fn the_tree_has_a_menu_on_a_right_press(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    cx.simulate_mouse_down(row.center(), gpui_kit::MouseButton::Right, gpui_kit::Modifiers::default());
    settle(&shell, cx);
    for item in ["tree-new-file", "tree-new-folder", "tree-rename", "tree-duplicate", "tree-delete", "tree-copy-path", "tree-copy-relative", "tree-mention"] {
        assert!(cx.debug_bounds(item).is_some(), "the file's menu has {item}");
    }
    assert!(cx.debug_bounds("tree-collapse").is_none(), "a file has no folders to collapse");
}

/// Each row has a ⋯ button that opens the row's menu, and the press does not open the folder or the file.
#[gpui_kit::test]
fn a_tree_row_has_a_button_for_its_menu(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    let more = cx.debug_bounds("tree-more-a.txt").expect("the row has the button");
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    assert!(more.right() <= row.right() && more.left() > row.center().x, "the button is at the row's right end");
    cx.simulate_mouse_move(row.center(), None, gpui_kit::Modifiers::default());
    cx.simulate_mouse_move(more.center(), None, gpui_kit::Modifiers::default());
    settle(&shell, cx);
    cx.simulate_mouse_down(more.center(), gpui_kit::MouseButton::Left, gpui_kit::Modifiers::default());
    cx.simulate_mouse_up(more.center(), gpui_kit::MouseButton::Left, gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("tree-mention").is_some(), "the menu opens, with Mention in session");
    assert!(cx.debug_bounds("editor-tab-0").is_none(), "and the file did not open");
}

/// A name typed in the tree ends when the reader presses Escape, or presses anywhere in the tree but on the name itself.
#[gpui_kit::test]
fn a_name_in_the_tree_ends_on_escape_and_on_a_press_elsewhere(cx: &mut TestAppContext) {
    use crate::open_project::tree_edit::TreeEditKind;
    let (shell, cx, dir) = with_a_session(cx, 1600.);
    open_files_from_the_menu(&shell, cx);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    let editing = |cx: &mut gpui_kit::VisualTestContext| project.read_with(cx, |p, _| p.tree_edit.is_some());
    project.update_in(cx, |p, window, cx| p.start_tree_edit(TreeEditKind::Rename { path: "a.txt".into() }, window, cx));
    settle(&shell, cx);
    assert!(editing(cx), "the name is being typed");
    cx.simulate_keystrokes("escape");
    settle(&shell, cx);
    assert!(!editing(cx), "Escape ends it");
    assert!(dir.path().join("a.txt").exists(), "and nothing was renamed");
    project.update_in(cx, |p, window, cx| p.start_tree_edit(TreeEditKind::NewFile { parent: String::new() }, window, cx));
    settle(&shell, cx);
    let name = cx.debug_bounds("tree-edit-row").expect("the name has a row");
    cx.simulate_click(name.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(editing(cx), "a press on the name itself keeps it");
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(!editing(cx), "a press on another row ends it");
}

/// Gives the front project's one session a finished turn that changed `files` (names relative to the project): the real tracker takes
/// each file's text, the file changes, and the turn is pushed to the session's review.
fn session_changed(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext, dir: &std::path::Path, files: &[&str]) {
    use atelier_agents::session::{Event, ToolCall, ToolId, ToolKind, ToolStatus};
    let (project, session) = shell.read_with(cx, |s, cx| {
        let project = s.active().cloned().unwrap();
        let session = project.read(cx).sessions[0].clone();
        (project, session)
    });
    let host = project.read_with(cx, |p, _| p.host());
    let mut tracker = atelier_review::TurnTracker::begin(host.as_ref());
    for (n, file) in files.iter().enumerate() {
        std::fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
        tracker.observe(host.as_ref(), &Event::ToolStarted(ToolCall {
            id: ToolId::new(format!("t{n}")),
            name: "Write".into(),
            kind: ToolKind::Write,
            input: serde_json::Value::Null,
            file: Some(dir.join(file).display().to_string()),
            parent: None,
            status: ToolStatus::Running,
        }));
        std::fs::write(dir.join(file), format!("changed {n}\n")).unwrap();
    }
    let turn = tracker.finish(host.as_ref());
    cx.update(|_, cx| session.update(cx, |s, cx| {
        s.reviews.turns.push(turn);
        s.diff_session(cx);
    }));
    settle(shell, cx);
}

/// The Files tree marks a file a session changed, and the folders above it, and a press on the mark reviews that session's changes.
#[gpui_kit::test]
fn the_files_tree_marks_what_a_session_changed_and_a_press_reviews_it(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1600.);
    session_changed(&shell, cx, dir.path(), &["a.txt", "src/deep/b.rs"]);
    let project = shell.read_with(cx, |s, _| s.active().cloned().unwrap());
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(&project, move |_, event: &crate::open_project::ProjectEvent, _| {
            if let crate::open_project::ProjectEvent::Review { turn, path, .. } = event {
                log.borrow_mut().push((*turn, path.clone()));
            }
        })
        .detach()
    });
    project.update(cx, |p, cx| p.relist(cx));
    open_files_from_the_menu(&shell, cx);
    assert!(cx.debug_bounds("tree-mark-a.txt").is_some(), "the file the session changed has a mark");
    assert!(cx.debug_bounds("tree-mark-src").is_some(), "and so has the folder that holds a file it changed");
    let mark = cx.debug_bounds("tree-mark-a.txt").unwrap();
    cx.simulate_click(mark.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(heard.borrow().as_slice(), [(None, Some("a.txt".to_string()))], "a press reviews the session's changes on that file");
    assert!(cx.debug_bounds("editor-tab-0").is_none(), "and does not open the file");
}

/// A file no session changed has no mark; the menu of a changed file offers the review by the session that changed it.
#[gpui_kit::test]
fn an_unchanged_file_has_no_mark_and_a_changed_one_has_a_review_row_in_its_menu(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1600.);
    std::fs::write(dir.path().join("other.txt"), "x").unwrap();
    session_changed(&shell, cx, dir.path(), &["a.txt"]);
    open_files_from_the_menu(&shell, cx);
    settle(&shell, cx);
    assert!(cx.debug_bounds("tree-mark-other.txt").is_none(), "no session changed it");
    let row = cx.debug_bounds("tree-row-a.txt").expect("the tree lists a.txt");
    cx.simulate_mouse_down(row.center(), gpui_kit::MouseButton::Right, gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("tree-review-session").is_some(), "the menu offers the review by that session");
}

/// The tabs leave room for the ⋯ of the layout menu, where it stands.
#[test]
fn the_tabs_leave_room_for_the_layout_menu() {
    use super::helpers::tab_room;
    // The session area reaches the window's edge: the ⋯ is left of the Settings button.
    assert_eq!(tab_room(1400., Some(1400.)), 32.);
    // A pane at the right shortens the area: the ⋯ is at its right edge, and the tabs end before it.
    assert_eq!(tab_room(1400., Some(1000.)), 1400. - 48. - (1000. - 36. - 8.));
    assert_eq!(tab_room(1400., None), 36.);
}

/// The sidebar lists by project, or in one list by priority with a heading for each section; the choice is behind
/// the ⋯ in its head.
#[gpui_kit::test]
fn the_sidebar_switches_between_projects_and_the_priority_list(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let press = |name: &'static str, cx: &mut gpui_kit::VisualTestContext| {
        let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
        cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
        settle(&shell, cx);
    };
    assert!(cx.debug_bounds("section-Earlier").is_none(), "by project: no headings");
    press("sidebar-options", cx);
    press("list-mode-priority", cx);
    assert!(cx.debug_bounds("section-Earlier").is_some(), "by priority: the session is under Earlier");
    assert!(cx.debug_bounds("row-project").is_some(), "and its row wears the project's badge");
    press("sidebar-options", cx);
    let projects = cx.debug_bounds("list-mode-projects").unwrap();
    cx.simulate_click(projects.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("section-Earlier").is_none() && cx.debug_bounds("row-project").is_none(), "back by project");
}

/// The Settings page changes the sidebar's look and the sidebar keeps the mode and the filter the head chose.
#[gpui_kit::test]
fn a_look_from_settings_reaches_the_sidebar_and_keeps_the_heads_choices(cx: &mut TestAppContext) {
    use atelier_ui::{sidebar_layout::{BadgeShow, SidebarLayout}, sidebar_model::ListMode};
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    shell.update(cx, |s, cx| s.agents_sidebar.update(cx, |sb, cx| sb.choose_mode(ListMode::Priority, cx)));
    let look = SidebarLayout { project_badge: BadgeShow::Never, show_time: false, ..Default::default() };
    shell.update_in(cx, |s, window, cx| {
        s.open_settings(&OpenSettings, window, cx);
        let pane = s.settings.as_ref().unwrap().0.clone();
        pane.update(cx, |_, cx| cx.emit(crate::settings_pane::SettingsEvent::Sidebar(look)));
    });
    settle(&shell, cx);
    let layout = shell.read_with(cx, |s, cx| s.agents_sidebar.read(cx).layout());
    assert_eq!((layout.project_badge, layout.show_time), (BadgeShow::Never, false));
    assert_eq!(layout.mode, ListMode::Priority, "the head's mode stays");
}

/// ⌘+, ⌘− and ⌘0 scale the whole interface, as Zed's do: a button is as wide as the zoom says, and the window's breakpoints
/// follow the scaled width.
#[gpui_kit::test]
fn the_zoom_keys_scale_every_size_together(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    let (zoom_in, zoom_out, reset) = if cfg!(target_os = "macos") { ("cmd-=", "cmd--", "cmd-0") } else { ("ctrl-=", "ctrl--", "ctrl-0") };
    let width = |cx: &mut gpui_kit::VisualTestContext| f32::from(cx.debug_bounds("settings-entry").expect("the Settings button").size.width);
    let base = width(cx);
    for _ in 0..5 {
        cx.simulate_keystrokes(zoom_in);
    }
    settle(&shell, cx);
    assert!((atelier_ui::scale::zoom() - 1.5).abs() < 1e-4, "five presses of ⌘+ are 1.5: {}", atelier_ui::scale::zoom());
    assert!((width(cx) - base * 1.5).abs() < 1.5, "the button is 1.5 times as wide: {} against {base}", width(cx));
    for _ in 0..30 {
        cx.simulate_keystrokes(zoom_out);
    }
    assert_eq!(atelier_ui::scale::zoom(), atelier_ui::scale::MIN, "⌘− stops at the least");
    cx.simulate_keystrokes(reset);
    settle(&shell, cx);
    assert_eq!(atelier_ui::scale::zoom(), 1.);
    assert!((width(cx) - base).abs() < 0.5, "⌘0 puts it back");
}

/// A long conversation: the rail shows a tick for each message sent, the pointer on a tick shows its card, a press scrolls to
/// that message and lets go of the end (so "Latest" shows), and "Latest" takes hold of the end again.
#[gpui_kit::test]
fn a_long_conversation_has_a_rail_and_a_latest_button(cx: &mut TestAppContext) {
    use atelier_agents::session::{BlockId, Event};
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let session = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].clone());
    assert!(cx.debug_bounds("message-rail").is_none(), "no rail before there is something to scroll");
    session.update(cx, |s, cx| {
        for n in 0..30 {
            s.conversation.user_sent(format!("message number {n}"));
            s.conversation.apply(&Event::Text { block: BlockId(n), delta: format!("answer to {n}") });
        }
        s.refresh_rows();
        cx.notify();
    });
    settle(&shell, cx);
    settle(&shell, cx);
    assert!(cx.debug_bounds("message-rail").is_some(), "a long conversation has a rail");
    assert!(cx.debug_bounds("rail-tick-29").is_some() && cx.debug_bounds("rail-tick-0").is_some(), "a tick for each message");
    assert!(cx.debug_bounds("latest").is_none(), "following the output: no Latest button");
    let tick = cx.debug_bounds("rail-tick-3").unwrap();
    cx.simulate_mouse_move(tick.center(), None, gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("rail-card").is_some(), "the pointer on a tick shows its card");
    cx.simulate_click(tick.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("latest").is_some(), "a press on a tick lets go of the end, so Latest shows");
    let view = session.read_with(cx, |s, _| s.list.viewport_bounds());
    let message = cx.debug_bounds("row-6").expect("message 3 is in view");
    assert!((message.center().y - view.center().y).abs() < px(2.), "and it is in the middle: {message:?} in {view:?}");
    let latest = cx.debug_bounds("latest").unwrap();
    cx.simulate_click(latest.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("latest").is_none(), "Latest takes hold of the end again");
}

/// With motion on, a press on a tick glides to its message over a few frames, as beui's smooth scroll does, and a message
/// sent then rises into its place.
#[gpui_kit::test]
fn a_jump_glides_and_a_new_message_rises_in(cx: &mut TestAppContext) {
    use atelier_agents::session::{BlockId, Event};
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let session = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].clone());
    session.update(cx, |s, cx| {
        for n in 0..30 {
            s.conversation.user_sent(format!("message number {n}"));
            s.conversation.apply(&Event::Text { block: BlockId(n), delta: format!("answer to {n}") });
        }
        s.refresh_rows();
        s.arrived.clear();
        cx.notify();
    });
    settle(&shell, cx);
    settle(&shell, cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    let tick = cx.debug_bounds("rail-tick-3").unwrap();
    cx.simulate_click(tick.center(), gpui_kit::Modifiers::default());
    let view = session.read_with(cx, |s, _| s.list.viewport_bounds());
    let off = |cx: &mut gpui_kit::VisualTestContext| cx.debug_bounds("row-6").map_or(f32::MAX, |m| f32::from((m.center().y - view.center().y).abs()));
    cx.run_until_parked();
    assert!(off(cx) > 2., "one frame in, it is still on its way");
    let mut frames = 0;
    while off(cx) > 2. && frames < 60 {
        std::thread::sleep(std::time::Duration::from_millis(16));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        frames += 1;
    }
    assert!(off(cx) <= 2., "it lands in the middle");
    assert!(frames > 3, "over several frames, not at once: {frames}");
    // Back at the end, as a reader who sends is.
    session.update(cx, |s, cx| {
        s.glide.follow();
        cx.notify();
    });
    let at_end = |cx: &mut gpui_kit::VisualTestContext| {
        let last = session.read_with(cx, |s, _| s.shown.len() - 1);
        cx.debug_bounds(format!("row-{last}").leak()).is_some_and(|r| r.bottom() <= view.bottom() + px(1.))
    };
    frames = 0;
    while !at_end(cx) && frames < 120 {
        std::thread::sleep(std::time::Duration::from_millis(8));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        frames += 1;
    }
    session.update(cx, |s, cx| {
        s.conversation.user_sent("one more");
        s.refresh_rows();
        // The rise runs on the wall clock; a machine busy with other tests can draw the first frame after it ended. An arrival
        // dated ahead is still at its start, whenever that frame is drawn.
        s.arrived.values_mut().for_each(|at| *at += std::time::Duration::from_secs(30));
        s.glide.follow();
        cx.notify();
    });
    cx.run_until_parked();
    // The message is followed by the waiting line, which holds the agent's place until it answers.
    let last = session.read_with(cx, |s, _| {
        assert_eq!(s.shown.last(), Some(&crate::list_diff::Row::Waiting));
        s.shown.len() - 2
    });
    let row_selector: &'static str = format!("row-{last}").leak();
    for _ in 0..20 {
        if cx.debug_bounds(row_selector).is_some() {
            break;
        }
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let (row, rising) = (cx.debug_bounds(format!("row-{last}").leak()), cx.debug_bounds(format!("entering-{last}").leak()));
    let (row, rising) = (row.expect("the new message is drawn"), rising.expect("and it is entering"));
    assert!(rising.top() > row.top(), "it starts below its place: {rising:?} in {row:?}");
}

/// A review opens in the Git view: in place of the panels, with the checkout's changes in the sidebar, and
/// the right pane as it was; when it closes, Sessions comes back.
#[gpui_kit::test]
fn a_review_opens_in_the_git_view_and_closing_it_goes_back(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let right_before = shell.read_with(cx, |s, _| (s.right, s.right_width));
    assert!(cx.debug_bounds("panel-close").is_some() && cx.debug_bounds("review-in-place").is_none());
    let session = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).sessions[0].clone());
    session.update(cx, |_, cx| cx.emit(crate::agent_session::SessionEvent::Review { turn: Some(0), path: None }));
    settle(&shell, cx);
    assert!(cx.debug_bounds("review-in-place").is_some(), "the review is where the panels were");
    assert!(cx.debug_bounds("panel-close").is_none(), "the panels make way");
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Git);
    assert!(cx.debug_bounds("changes-list").is_some(), "the sidebar lists the checkout's changes");
    assert_eq!(shell.read_with(cx, |s, _| (s.right, s.right_width)), right_before, "the right pane neither opens nor widens");
    let pane = shell.read_with(cx, |s, cx| s.active().cloned().unwrap().read(cx).review.as_ref().map(|(p, _)| p.clone()).unwrap());
    pane.update(cx, |_, cx| cx.emit(crate::review_pane::PaneEvent::Close));
    settle(&shell, cx);
    assert!(cx.debug_bounds("review-in-place").is_none() && cx.debug_bounds("panel-close").is_some(), "closed: the panels are back");
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Sessions);
}

/// The rail goes Tasks, Sessions, Git. A press on another view shows it; a press on the view in front
/// hides the sidebar, and the next press shows it again.
#[gpui_kit::test]
fn the_rail_switches_views_and_a_second_press_hides_the_sidebar(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let press = |name: &'static str, cx: &mut gpui_kit::VisualTestContext| {
        let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
        cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
        settle(&shell, cx);
    };
    let rail = cx.debug_bounds("view-rail").expect("the rail is drawn");
    let sidebar = cx.debug_bounds("sidebar").expect("the sidebar is drawn");
    assert!(rail.right() <= sidebar.left(), "the rail is left of the sidebar: {rail:?} {sidebar:?}");
    let tasks = cx.debug_bounds("rail-tasks").unwrap();
    let git = cx.debug_bounds("rail-git").unwrap();
    assert!(cx.debug_bounds("rail-sessions").unwrap().top() < tasks.top() && tasks.top() < git.top(), "Sessions, Issues, Code");

    press("rail-tasks", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Tasks);
    assert!(cx.debug_bounds("tasks-view").is_some() && cx.debug_bounds("panel-close").is_none(), "the board is in the main area");
    assert!(cx.debug_bounds("issues-sidebar").is_some(), "the sidebar holds the views over the issues, not the sessions");
    press("issues-backlog", cx);
    let scope = shell.read_with(cx, |s, cx| s.active().and_then(|p| p.read(cx).tasks.as_ref()).map(|t| t.pane.read(cx).scope().clone()));
    assert_eq!(scope, Some(crate::tasks::pane::Scope::Backlog), "a press on a view sets the pane's scope");
    press("rail-tasks", cx);
    assert!(!shell.read_with(cx, |s, _| s.sidebar), "a second press hides the sidebar");
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Tasks, "and the view stays");
    press("rail-tasks", cx);
    assert!(shell.read_with(cx, |s, _| s.sidebar), "a third shows it again");

    press("rail-git", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Files, "the Code lens opens on its files first");
    press("code-nav-git", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Git);
    assert!(cx.debug_bounds("changes-list").is_some() && cx.debug_bounds("review-in-place").is_none(), "the checkout's changes, no review");
    press("rail-sessions", cx);
    press("rail-git", cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Git, "the lens comes back on the view it was left on");
    press("rail-git", cx);
    press("rail-sessions", cx);
    assert!(shell.read_with(cx, |s, _| s.sidebar), "another view comes with the sidebar");
    assert!(cx.debug_bounds("panel-close").is_some(), "the panels are back");
}

/// The project menu's Worktrees opens the Git view, which lists the main checkout and each worktree.
#[gpui_kit::test]
fn the_project_menu_opens_the_worktrees_in_the_git_view(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    std::fs::create_dir(&main).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&main)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(main.join("a.txt"), "a").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "first"]);
    let fix = dir.path().join("fix");
    git(&["worktree", "add", "-q", "-b", "fix", fix.to_str().unwrap()]);
    std::fs::write(fix.join("b.txt"), "b").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1400.), px(900.)));
    shell.update_in(cx, |s, window, cx| s.open_local(main.clone(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| {
        let project = crate::agents_view::project_id(s.active().unwrap().read(cx));
        let sidebar = s.agents_sidebar.clone();
        s.sidebar_event(&sidebar, &atelier_ui::sidebar::SidebarEvent::Worktrees { project }, window, cx);
    });
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Git);
    assert!(cx.debug_bounds("worktrees").is_some(), "the Git view lists the worktrees");
    let rows = shell.read_with(cx, |s, cx| s.active().unwrap().read(cx).worktree_rows());
    let seen: Vec<(Option<&str>, bool)> = rows.iter().map(|r| (r.branch.as_deref(), r.main)).collect();
    assert_eq!(seen, [(Some("main"), true), (Some("fix"), false)]);
    assert!(rows[1].notes.iter().any(|n| n.words == "1 uncommitted"), "{:?}", rows[1].notes);
    let main_row = std::fs::canonicalize(&main).unwrap().to_string_lossy().into_owned();
    assert!(cx.debug_bounds(format!("worktree-{main_row}").leak()).is_some());
}

#[test]
fn a_project_that_needs_the_reader_outweighs_one_at_work() {
    use atelier_ui::session_status::{Need, SessionStatus};
    use super::helpers::{ProjectMark, project_mark};
    assert_eq!(project_mark(&[]), ProjectMark::Quiet);
    assert_eq!(project_mark(&[SessionStatus::Idle, SessionStatus::Working]), ProjectMark::Working);
    assert_eq!(project_mark(&[SessionStatus::Working, SessionStatus::NeedsYou(Need::Question)]), ProjectMark::NeedsYou);
}

/// The switcher at the head of the sidebar: in Sessions it narrows the list and the panels to one project or shows all of
/// them; in the Code lens it is the project the view is about.
#[gpui_kit::test]
fn the_project_switcher_narrows_sessions_and_names_the_project_of_code(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1600.);
    assert!(cx.debug_bounds("project-switcher").is_some(), "the switcher is drawn");
    assert_eq!(shell.read_with(cx, |s, cx| s.switched(cx)), None, "Sessions shows every project at first");
    let press = |name: &'static str, cx: &mut gpui_kit::VisualTestContext| {
        let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
        cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
        settle(&shell, cx);
    };
    press("project-switcher", cx);
    assert!(cx.debug_bounds("switcher-all").is_some(), "Sessions offers all projects");
    let name = shell.read_with(cx, |s, cx| s.active().unwrap().read(cx).name());
    let row_name: &'static str = Box::leak(format!("switcher-{name}").into_boxed_str());
    let row = cx.debug_bounds(row_name).expect("each project has a row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.switched(cx)), Some(0), "the list is narrowed to it");
    assert!(cx.debug_bounds("panel-close").is_some(), "its panels stay");
    press("project-switcher", cx);
    press("switcher-all", cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.switched(cx)), None, "and back to all of them");

    press("rail-git", cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.switched(cx)), Some(0), "Code is about one project");
    press("project-switcher", cx);
    assert!(cx.debug_bounds("switcher-all").is_none(), "with no All projects");
}

/// The History view lists the branch's commits, newest first, and shows the newest in full until another is
/// picked.
#[gpui_kit::test]
fn history_lists_the_commits_and_shows_the_newest(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(["-c", "user.name=Ada", "-c", "user.email=ada@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "First"]);
    std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "bee\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "Second\n\nWhy it changed."]);
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1400.), px(900.)));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go_to(ShellView::History, window, cx));
    settle(&shell, cx);
    settle(&shell, cx);
    let (subjects, picked) = shell.read_with(cx, |s, cx| {
        let p = s.active().unwrap().read(cx);
        let subjects = match &p.log {
            Some(crate::history::Read::Ready(commits)) => commits.iter().map(|c| c.subject.to_string()).collect::<Vec<_>>(),
            other => panic!("the log is read: {other:?}"),
        };
        let picked = match &p.commit {
            Some((_, crate::history::Read::Ready(shown))) => (shown.message.to_string(), shown.files.len()),
            other => panic!("the newest commit is read: {other:?}"),
        };
        (subjects, picked)
    });
    assert_eq!(subjects, ["Second", "First"]);
    assert_eq!(picked, ("Second\n\nWhy it changed.".to_string(), 2));
    assert!(cx.debug_bounds("history-list").is_some() && cx.debug_bounds("history-commit").is_some(), "the list and the commit are drawn");
    assert!(cx.debug_bounds("code-nav-history").is_some(), "the Code sidebar names the view");
    assert!(cx.debug_bounds("history-tree").is_some(), "the commit's files are a tree");
    assert!(cx.debug_bounds("file-diff-a.txt").is_some() && cx.debug_bounds("file-diff-b.txt").is_none(), "one file at a time, the tree's first");
    shell.update(cx, |s, cx| s.pick_file(ShellView::History, "b.txt".into(), cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("file-diff-b.txt").is_some() && cx.debug_bounds("file-diff-a.txt").is_none(), "the file picked in the tree");
}

/// A project over SSH says where it lives in the switcher, on the face and on its row, as the sidebar does; a
/// local one says nothing of the kind.
#[gpui_kit::test]
fn the_switcher_names_the_host_of_a_project_over_ssh(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1600.);
    shell.update_in(cx, |s, window, cx| s.go_to(ShellView::Files, window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("switcher-host").is_none(), "a local project has no host");
    let remote = tempfile::tempdir().unwrap();
    let project = atelier_project::LocalProject::open(remote.path().to_path_buf()).unwrap();
    let location = atelier_settings::Location::Ssh { host: "pro".into(), path: remote.path().to_path_buf() };
    shell.update_in(cx, |s, window, cx| {
        s.add(location, std::sync::Arc::new(project), window, cx);
        s.active = s.projects.len() - 1;
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("switcher-host").is_some(), "the face names the host");
    let face = cx.debug_bounds("project-switcher").unwrap();
    cx.simulate_click(face.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    let row = cx.debug_bounds("switcher-host").unwrap();
    assert!(row.top() > face.bottom(), "and so does the project's row in the menu: {row:?} under {face:?}");
    drop(dir);
}

/// Changes is about the project's checkout, not a session: it lists the files the checkout holds uncommitted,
/// tracked or new, and shows each one's diff.
#[gpui_kit::test]
fn changes_lists_what_the_checkout_holds_uncommitted(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(["-c", "user.name=Ada", "-c", "user.email=ada@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "First"]);
    std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    std::fs::write(dir.path().join("new.txt"), "fresh\n").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1400.), px(900.)));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go_to(ShellView::Git, window, cx));
    settle(&shell, cx);
    settle(&shell, cx);
    let paths = shell.read_with(cx, |s, cx| match &s.active().unwrap().read(cx).uncommitted {
        Some(crate::history::Read::Ready(files)) => files.iter().map(|f| f.path.to_string()).collect::<Vec<_>>(),
        other => panic!("the changes are read: {other:?}"),
    });
    assert_eq!(paths, ["a.txt", "new.txt"], "the changed file, then the new one");
    assert!(cx.debug_bounds("changes-list").is_some() && cx.debug_bounds("changes-diffs").is_some(), "the list and the diffs are drawn");
    assert!(cx.debug_bounds("changes-tree").is_some(), "the sidebar's files are a tree");
    assert!(cx.debug_bounds("file-diff-a.txt").is_some() && cx.debug_bounds("file-diff-new.txt").is_none(), "one file at a time, the tree's first");
    shell.update(cx, |s, cx| s.pick_file(ShellView::Git, "new.txt".into(), cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("file-diff-new.txt").is_some() && cx.debug_bounds("file-diff-a.txt").is_none(), "the file picked in the tree");
}

/// A commit leaves `.git` alone in the file tree, so the watch says nothing: reading git again must read the checkout
/// again, or Changes keeps listing files that are committed.
#[gpui_kit::test]
fn changes_empties_when_git_is_read_again_after_a_commit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(["-c", "user.name=Ada", "-c", "user.email=ada@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("a.txt"), "one
").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "First"]);
    std::fs::write(dir.path().join("a.txt"), "two
").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.simulate_resize(size(px(1400.), px(900.)));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go_to(ShellView::Git, window, cx));
    settle(&shell, cx);
    settle(&shell, cx);
    let listed = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| {
        shell.read_with(cx, |s, cx| match &s.active().unwrap().read(cx).uncommitted {
            Some(crate::history::Read::Ready(files)) => files.len(),
            other => panic!("the changes are read: {other:?}"),
        })
    };
    assert_eq!(listed(&shell, cx), 1);
    git(&["commit", "-qam", "Second"]);
    shell.update(cx, |s, cx| s.active().unwrap().update(cx, |p, cx| p.refresh_git(cx)));
    settle(&shell, cx);
    settle(&shell, cx);
    assert_eq!(listed(&shell, cx), 0, "the committed file is not listed");
}

/// The switcher heads the sidebar, under the title bar and above the sessions, and the title bar keeps the sidebar's
/// toggle. The add button is as wide as it is tall: an icon alone has no words to part from.
#[gpui_kit::test]
fn the_switcher_heads_the_sidebar_and_its_add_button_is_square(cx: &mut TestAppContext) {
    let (_shell, cx, _dir) = with_a_session(cx, 1400.);
    let switcher = cx.debug_bounds("project-switcher").expect("the switcher is drawn");
    let toggle = cx.debug_bounds("sidebar-toggle").expect("the toggle is drawn");
    assert!(switcher.top() >= gpui_kit::px(super::types::TITLE_BAR * atelier_ui::scale::zoom()), "under the title bar: {switcher:?}");
    assert!(toggle.bottom() <= switcher.top(), "the toggle stays in the title bar: {toggle:?}");
    let add = cx.debug_bounds("add-project").expect("the add button is drawn");
    assert_eq!(add.size.width, add.size.height, "a square: {add:?}");
}

/// The bar at the foot of the window shows once a project is open: the machine, and the sessions' work.
#[gpui_kit::test]
fn the_foot_of_the_window_has_the_status_bar_once_a_project_is_open(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    shell.update(cx, |shell, cx| shell.sample_vitals(cx));
    settle(&shell, cx);
    let bar = cx.debug_bounds("status-bar").expect("the bar is drawn");
    assert!(cx.debug_bounds("status-cpu").is_some() && cx.debug_bounds("status-memory").is_some(), "the machine shows after a sample");
    let panes = cx.debug_bounds("sessions-view").expect("the panes are drawn");
    let (machine, main) = (cx.debug_bounds("status-card-cpu").expect("a card under the sidebar"), cx.debug_bounds("status-card-main").expect("a card under the session"));
    // The first card stands under the sidebar: past the rail and the panels' gap, not under the rail. The next is a panels' gap on.
    let first = f32::from(panes.left()) + atelier_ui::view_rail::WIDTH + super::types::PANE_GAP;
    assert!((f32::from(machine.left()) - first).abs() < 0.6, "the first card starts at {:?}, not past the rail", machine.left());
    assert!((f32::from(main.left() - machine.right()) - super::types::PANE_GAP).abs() < 0.6, "a panels' gap between the cards");
    assert!((f32::from(bar.right()) - (f32::from(panes.right()) - 8.)).abs() < 0.6, "the last card ends 8 px from the window's right edge");
    // The bar's room at the foot is the room at the right edge.
    let height = cx.update(|window, _| window.viewport_size().height);
    assert_eq!(bar.bottom(), height - gpui_kit::px(8.), "8 px above the foot: {bar:?} in a window {height:?} tall");
}

#[gpui_kit::test]
fn the_start_screen_has_no_status_bar(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    settle(&shell, cx);
    assert!(cx.debug_bounds("status-bar").is_none());
}

/// Sessions at work and sessions that wait on the reader are counted over every project.
#[gpui_kit::test]
fn work_counts_the_sessions_at_work_and_the_ones_that_wait(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    assert_eq!(shell.read_with(cx, |s, cx| s.work(cx)), atelier_ui::Work::new(0, 0), "an idle session is neither");
}

/// A colour the reader picks for a project's letter badge is the one its badge wears from then on.
#[gpui_kit::test]
fn a_colour_picked_for_a_project_is_kept_for_its_badge(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let place = shell.read_with(cx, |s, cx| crate::agents_view::project_id(s.projects[0].read(cx)));
    shell.update(cx, |s, cx| s.save_color(place.clone(), 9, cx));
    settle(&shell, cx);
    let kept = shell.read_with(cx, |s, _| s.badges.colors.get(place.as_ref()).copied());
    assert_eq!(kept, Some(9));
    let worn = shell.read_with(cx, |s, cx| s.project_badges(cx)[0].color);
    assert_eq!(worn, 9, "the sidebar's badge wears it");
}

/// A long project name on a long host name stays inside the sidebar: the name is cut, the add button stays whole.
#[gpui_kit::test]
fn a_long_project_name_on_a_long_host_stays_inside_the_sidebar_head(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("fluentai-pro-with-a-name-that-is-long-indeed");
    std::fs::create_dir(&root).unwrap();
    let project = atelier_project::LocalProject::open(root.clone()).unwrap();
    let location = atelier_settings::Location::Ssh { host: "hp-agent-on-the-other-side".into(), path: root };
    shell.update_in(cx, |s, window, cx| {
        s.add(location, std::sync::Arc::new(project), window, cx);
        s.active = s.projects.len() - 1;
        cx.notify();
    });
    shell.update_in(cx, |s, window, cx| s.go_to(ShellView::Files, window, cx));
    settle(&shell, cx);
    let add = cx.debug_bounds("add-project").expect("the add button is drawn");
    let face = cx.debug_bounds("project-switcher").expect("the face is drawn");
    let sidebar = px(super::fit::SIDEBAR_DEFAULT * atelier_ui::scale::zoom());
    assert!(add.right() <= sidebar + px(48.), "the add button is inside the sidebar: {add:?} in {sidebar:?}");
    assert!(face.right() <= add.left() + px(2.), "the face leaves the add button its place: {face:?} {add:?}");
}

mod updates;

/// A script presses the Settings button and a section by name, with no pointer, and the page answers.
#[gpui_kit::test]
fn a_script_opens_the_providers_page_by_pressing_names(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let press = |name: &str, cx: &mut gpui_kit::VisualTestContext| {
        let at = cx.update(|_, cx| crate::control::find(name, cx)).unwrap_or_else(|| panic!("{name} was not drawn")).center();
        cx.update(|window, cx| crate::control::press_in_steps(window, at, cx));
        settle(&shell, cx);
        settle(&shell, cx);
    };
    assert_eq!(shell.read_with(cx, |s, cx| s.settings_section(cx)), None, "the page starts closed");
    press("settings-entry", cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.settings_section(cx)), Some("section-appearance"));
    press("section-providers", cx);
    assert_eq!(shell.read_with(cx, |s, cx| s.settings_section(cx)), Some("section-providers"));
}

/// The count on the Changes row is the checkout's once it is read: a clean checkout shows none whatever the session changed.
#[test]
fn the_changes_count_is_the_checkouts_once_it_is_read() {
    use crate::history::{CommitFile, Read};
    let file = |path: &str| CommitFile { path: path.to_string().into(), change: atelier_ui::FileChange::Modified, lines: vec![] };
    assert_eq!(super::helpers::changes_badge(None, 3), Some(3), "before git is read, the session's count");
    assert_eq!(super::helpers::changes_badge(Some(&Read::Reading), 0), None);
    assert_eq!(super::helpers::changes_badge(Some(&Read::Ready(vec![])), 3), None, "a clean checkout shows none");
    assert_eq!(super::helpers::changes_badge(Some(&Read::Ready(vec![file("a"), file("b")])), 0), Some(2));
}

/// The Sessions sidebar has one row at its head: the project picker at the left and the ⋯ at the right, on the same line.
#[gpui_kit::test]
fn the_project_picker_and_the_sidebar_options_share_one_row(cx: &mut TestAppContext) {
    let (_shell, cx, _dir) = with_a_session(cx, 1400.);
    let picker = cx.update(|_, cx| crate::control::find("project-switcher", cx)).expect("the picker is drawn");
    let options = cx.debug_bounds("sidebar-options").expect("the ⋯ is drawn");
    let (a, b) = (f32::from(picker.center().y), f32::from(options.center().y));
    assert!((a - b).abs() <= 2., "the picker's middle is at {a} and the ⋯'s at {b}");
    assert!(picker.right() < options.left(), "the picker is left of the ⋯: {picker:?} {options:?}");
}
