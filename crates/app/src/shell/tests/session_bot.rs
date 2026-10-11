//! Sessions that belong to a bot: one starts by the bot's id or from its profile, its row and its header show the bot's
//! face, the settings keep whose it is, and a session started the old way has none of it.
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::super::{Shell, ShellView};
use super::{settle, with_a_session};
use crate::{bots_view::BotsPlugin, session_bot::BotsFolder, slots::Slots};

/// An app whose bots live in `root`: the Bots view reads them there, and so does a session that belongs to one.
fn keeping_bots_in(root: Option<std::path::PathBuf>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        cx.global_mut::<Slots>().plug(&BotsPlugin::at(root.clone()));
        cx.set_global(BotsFolder(root));
    });
}

/// The bot of each session of the project in front, in order.
fn bots_of_sessions(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<Option<String>> {
    shell.read_with(cx, |s, cx| {
        let project = s.active().unwrap().read(cx);
        project.sessions.iter().map(|session| session.read(cx).bot.as_ref().map(|b| b.id.to_string())).collect()
    })
}

/// A session started by a bot's id, as the control socket starts one: the bots live in a folder of the test.
fn start_as(shell: &Entity<Shell>, id: &str, root: &std::path::Path, cx: &mut VisualTestContext) -> Result<(), String> {
    keeping_bots_in(Some(root.to_path_buf()), cx);
    let started = shell.update_in(cx, |s, window, cx| s.new_session_of_bot(id, window, cx).map(|_| ()));
    settle(shell, cx);
    started
}

#[gpui_kit::test]
fn a_session_started_as_a_bot_records_it_shows_its_face_and_comes_in_front_in_sessions(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    assert_eq!(bots_of_sessions(&shell, cx), [None], "a session started the old way has no bot");
    assert!(cx.debug_bounds("session-mark").is_none(), "and its row keeps the agent's mark");
    assert!(cx.debug_bounds("panel-bot").is_none(), "as its header does");
    // From another lens, as the profile's button starts one.
    keeping_bots_in(Some(dir.path().join("bots")), cx);
    shell.update(cx, |s, cx| s.open_view("bots", cx));
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Plugin("bots"));
    start_as(&shell, "dot", &dir.path().join("bots"), cx).expect("Dot starts");
    assert_eq!(bots_of_sessions(&shell, cx), [None, Some("dot".to_string())]);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Sessions, "the Sessions view is in front");
    let front = shell.read_with(cx, |s, cx| s.front_session(cx).and_then(|session| session.read(cx).bot.as_ref().map(|b| b.id.to_string())));
    assert_eq!(front.as_deref(), Some("dot"), "with the bot's session open");
    let face = cx.debug_bounds("session-mark").expect("the row of the bot's session shows the app's mark: the face");
    assert_eq!(f32::from(face.size.width), atelier_ui::session_row::MARK_BOX, "in the box of the agent's mark");
    assert!(cx.debug_bounds("panel-bot").is_some(), "and the header shows the face too");
    let state = shell.read_with(cx, crate::control::state);
    let sessions = state["projects"][0]["sessions"].as_array().unwrap();
    assert!(sessions[0]["bot"].is_null(), "the control socket says the first session has no bot");
    assert_eq!((sessions[1]["bot"].as_str(), sessions[1]["mood"].as_str()), (Some("dot"), Some("idle")), "and that the second is Dot's, idle");
}

#[gpui_kit::test]
fn a_bot_nobody_keeps_starts_no_session_and_says_why(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    let why = start_as(&shell, "nobody", &dir.path().join("bots"), cx).unwrap_err();
    assert!(why.contains("nobody"), "{why}");
    assert_eq!(bots_of_sessions(&shell, cx), [None], "no session opened");
    keeping_bots_in(None, cx);
    let no_folder = shell.update_in(cx, |s, window, cx| s.new_session_of_bot("dot", window, cx).map(|_| ())).unwrap_err();
    assert!(no_folder.contains("no folder"), "{no_folder}");
}

/// A bot is a persona on its harness: one whose harness this build cannot start says so and opens nothing.
#[gpui_kit::test]
fn a_bot_on_a_harness_the_app_cannot_start_opens_no_session_and_says_so(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let mut bot = atelier_bots::starter_crew().into_iter().find(|b| b.id.as_str() == "dot").unwrap();
    bot.harness = atelier_bots::Harness::Grok;
    let why = shell.update_in(cx, |s, window, cx| s.new_session_as(bot, window, cx).map(|_| ())).unwrap_err();
    assert!(why.contains("Dot") && why.contains("Grok"), "{why}");
    assert_eq!(bots_of_sessions(&shell, cx), [None]);
}

#[gpui_kit::test]
fn a_press_on_start_a_session_in_a_profile_opens_a_session_of_that_bot_in_sessions(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    keeping_bots_in(Some(dir.path().join("bots")), cx);
    shell.update(cx, |s, cx| s.open_view("bots", cx));
    settle(&shell, cx);
    let dot = cx.debug_bounds("bot-row-dot").expect("the bots are listed");
    cx.simulate_click(dot.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    let start = cx.debug_bounds("bot-start-session").expect("the profile has its button");
    cx.simulate_click(start.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(bots_of_sessions(&shell, cx), [None, Some("dot".to_string())], "the bot of the profile, in the project in front");
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Sessions);
    let agent = shell.read_with(cx, |s, cx| s.front_session(cx).map(|session| session.read(cx).agent.name));
    assert!(agent.is_some(), "its session is the one in front");
    let marked = shell.read_with(cx, |_, cx| crate::control::find("bot-start-session", cx).is_some());
    assert!(marked, "a script presses the button by its name");
}

/// What the window keeps of its open sessions goes through the settings file and back; the next window opens the session
/// again as the bot's.
#[gpui_kit::test]
fn a_session_of_a_bot_is_still_the_bots_after_a_save_and_a_load_of_the_settings(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    let bots = dir.path().join("bots");
    start_as(&shell, "dot", &bots, cx).expect("Dot starts");
    // The window keeps its open sessions whenever it lists them again, as it does when one of them changes.
    shell.update(cx, |s, cx| s.sync(cx));
    let (open, front) = shell.read_with(cx, |s, _| s.saved_open.clone());
    // The newest session stands first, as its panel does.
    assert_eq!(open.iter().map(|o| o.bot.as_deref()).collect::<Vec<_>>(), [Some("dot"), None], "the window keeps each session's bot");
    let file = dir.path().join("settings.json");
    atelier_settings::update(&file, |s| (s.open, s.front) = (open.clone(), front.clone())).unwrap();
    let loaded = atelier_settings::load(&file);
    assert_eq!(loaded.open, open, "the file gives back what was kept");

    // The next launch: a window that knows only what the file holds.
    let (next, cx) = cx.add_window_view(|_, cx| Shell::new(&atelier_settings::Settings::default(), cx));
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(900.)));
    next.update_in(cx, |s, window, cx| s.restore(loaded.open, loaded.front, true, window, cx));
    settle(&next, cx);
    assert_eq!(bots_of_sessions(&next, cx), [Some("dot".to_string()), None], "the next launch still knows whose session it is");
    let resumed = next.read_with(cx, |s, cx| s.active().unwrap().read(cx).sessions[0].read(cx).id.as_ref().map(|id| id.as_str().to_string()));
    assert_eq!(resumed.as_deref(), Some("fake-1"), "and it is the same session of the agent, opened again");
}
