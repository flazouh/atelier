use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    sync::mpsc,
};

use futures_util::StreamExt;
use gpui_kit::{App, AnyWindowHandle, Context, Window, WeakEntity};
use serde_json::{Value, json};
use atelier_agents::session::Item;

use crate::{agent_session::AgentSession, list_diff::Row, shell::Shell};
use super::types::{Request, TEXT_KEPT};

/// Where the socket is. A debug build listens by default, at `atelier-<pid>.sock` in the runtime folder, so
/// a developer's app can always be driven; a release build listens only when `ATELIER_CONTROL` names a path.
/// `ATELIER_CONTROL=off` turns it off.
pub fn socket_path() -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    chosen(std::env::var_os("ATELIER_CONTROL"), cfg!(debug_assertions), &runtime, std::process::id())
}

pub(super) fn chosen(asked: Option<std::ffi::OsString>, debug: bool, runtime: &std::path::Path, pid: u32) -> Option<PathBuf> {
    match asked {
        Some(path) if path == "off" => None,
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ if debug => Some(runtime.join(format!("atelier-{pid}.sock"))),
        _ => None,
    }
}

type Call = (Request, mpsc::Sender<String>);

/// Listens at `path` and answers the requests on the UI thread, in the order they came.
pub fn serve(shell: WeakEntity<Shell>, window: AnyWindowHandle, path: PathBuf, cx: &mut App) {
    super::marks::enable();
    _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => return eprintln!("could not listen at {}: {error}", path.display()),
    };
    _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    let (calls, mut queue) = futures_channel::mpsc::unbounded::<Call>();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let calls = calls.clone();
            std::thread::spawn(move || talk(stream, calls));
        }
    });
    cx.spawn(async move |cx| {
        while let Some((request, reply)) = queue.next().await {
            let answer = window
                .update(cx, |_, window, cx| shell.update(cx, |shell, cx| handle(request, shell, window, cx)))
                .ok()
                .and_then(|answer| answer.ok())
                .unwrap_or_else(|| json!({ "error": "the window is gone" }));
            _ = reply.send(answer.to_string());
        }
    })
    .detach();
}

/// One client: each line it writes is a request, and each gets one line back.
fn talk(stream: std::os::unix::net::UnixStream, calls: futures_channel::mpsc::UnboundedSender<Call>) {
    let Ok(mut out) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Request>(&line) {
            Err(error) => json!({ "error": format!("not a request: {error}") }).to_string(),
            Ok(request) => {
                let (reply, wait) = mpsc::channel();
                if calls.unbounded_send((request, reply)).is_err() {
                    return;
                }
                match wait.recv() {
                    Ok(answer) => answer,
                    Err(_) => return,
                }
            }
        };
        if writeln!(out, "{answer}").is_err() {
            return;
        }
    }
}

fn handle(request: Request, shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> Value {
    match request {
        Request::State => state(shell, cx),
        Request::NewSession { agent } => {
            let agent = match agent {
                None => None,
                Some(name) => match atelier_agents::registry::agents().into_iter().find(|a| a.name.eq_ignore_ascii_case(&name)) {
                    Some(agent) => Some(agent),
                    None => return json!({ "error": format!("no agent named {name}") }),
                },
            };
            match shell.new_session_of(agent, window, cx) {
                Some(session) => json!({ "ok": true, "session": session.read(cx).key.as_ref() }),
                None => json!({ "error": "no project is open" }),
            }
        }
        Request::Limit => match shell.front_session(cx) {
            Some(session) => {
                use atelier_agents::session::{Event, Limit, LimitState, LimitWindow};
                let resets_at = Some(crate::agent_session::now() + 30 * 3600 + 39 * 60);
                session.update(cx, |s, cx| {
                    s.conversation.apply(&Event::Limit(Limit { state: LimitState::Reached, resets_at, window: Some(LimitWindow::Weekly) }));
                    cx.notify();
                });
                json!({ "ok": true, "session": session.read(cx).key.as_ref() })
            }
            None => json!({ "error": "no session is in front" }),
        },
        Request::Find { name } => match super::marks::find(&name, cx) {
            Some(b) => json!({ "x": f32::from(b.origin.x), "y": f32::from(b.origin.y), "w": f32::from(b.size.width), "h": f32::from(b.size.height) }),
            None => json!({ "error": format!("{name} has not been drawn") }),
        },
        Request::Click { name, x, y, button } => {
            let button = if button.as_deref() == Some("right") { gpui_kit::MouseButton::Right } else { gpui_kit::MouseButton::Left };
            let at = match (name, x, y) {
                (Some(name), _, _) => super::marks::find(&name, cx).map(|b| b.center()),
                (None, Some(x), Some(y)) => Some(gpui_kit::point(gpui_kit::px(x), gpui_kit::px(y))),
                _ => None,
            };
            match at {
                Some(position) => {
                    // The press runs once the shell is no longer being updated, so a handler can update it.
                    window.defer(cx, move |window, cx| press_with(window, position, button, cx));
                    json!({ "ok": true, "x": f32::from(position.x), "y": f32::from(position.y) })
                }
                None => json!({ "error": "click needs a name that has been drawn, or x and y" }),
            }
        }
        Request::Press { name } => match super::marks::find(&name, cx) {
            Some(b) => {
                let position = b.center();
                window.defer(cx, move |window, cx| press_in_steps(window, position, cx));
                json!({ "ok": true, "x": f32::from(position.x), "y": f32::from(position.y) })
            }
            None => json!({ "error": format!("{name} has not been drawn"), "marks": super::marks::names(cx) }),
        },
        Request::Marks => json!({ "marks": super::marks::names(cx) }),
        Request::View { name } => {
            // A view a plugin registered comes in front by its id, as a press on its entry of the rail does.
            if cx.global::<crate::slots::Slots>().view(&name).is_some() {
                shell.open_view(&name, cx);
                return json!({ "ok": true, "view": name });
            }
            let view = crate::shell::ShellView::from_words(Some(&name));
            if view.words() != name {
                return json!({ "error": format!("no view named {name}") });
            }
            shell.go_to(view, window, cx);
            json!({ "ok": true, "view": view.words() })
        }
        Request::Open { path, host } => {
            match host {
                Some(host) => shell.open_remote(host, path, window, cx),
                None => shell.open_local(path.into(), window, cx),
            }
            json!({ "ok": true })
        }
        Request::CheckUpdates => {
            shell.check_for_updates(&crate::shell::CheckForUpdates, window, cx);
            json!({ "ok": true })
        }
        Request::Update { event } => {
            shell.update_event(event, cx);
            json!({ "ok": true, "update": shell.update_state_word() })
        }
        Request::Edit { path, text } => {
            if shell.edit_file(&path, text, window, cx) {
                json!({ "ok": true })
            } else {
                json!({ "error": "no project is open" })
            }
        }
        Request::Send { text } => match shell.front_session(cx) {
            Some(session) => {
                session.update(cx, |s, cx| s.send(text, cx));
                json!({ "ok": true, "session": session.read(cx).key.as_ref() })
            }
            None => json!({ "error": "no session is in front" }),
        },
    }
}

/// The projects and sessions open, as a person would list them, the view in front, and the bots when their view was made.
pub(crate) fn state(shell: &Shell, cx: &App) -> Value {
    let front = shell.front_session(cx).map(|s| s.read(cx).key.to_string());
    let projects: Vec<Value> = shell
        .projects()
        .iter()
        .map(|project| {
            let project = project.read(cx);
            let sessions: Vec<Value> = project.sessions.iter().map(|s| {
                let s = s.read(cx);
                session_json(s, front.as_deref() == Some(s.key.as_ref()))
            }).collect();
            let listing = match &project.listing {
                crate::open_project::Listing::Loading => json!("loading"),
                crate::open_project::Listing::Ready(tree) => json!({ "entries": tree.len(), "ms": project.listed_in.map(|d| d.as_millis() as u64) }),
                crate::open_project::Listing::Failed(why) => json!({ "failed": why.as_ref() }),
            };
            json!({ "name": project.name(), "listing": listing, "sessions": sessions })
        })
        .collect();
    let theme = atelier_ui::theme::ActiveTheme::theme(cx);
    let updates = cx.try_global::<crate::updater::Updater>().is_some_and(crate::updater::Updater::available);
    let bots = shell.plugin_page::<crate::bots_view::BotsPage>("bots").map(|page| {
        let page = page.read(cx);
        let rows: Vec<&str> = page.entries().iter().map(|e| e.bot.id.as_str()).collect();
        json!({ "rows": rows, "chosen": page.selected().map(ToString::to_string), "error": page.error() })
    });
    json!({ "settings": shell.settings_section(cx), "unsaved": shell.unsaved(cx), "updates": { "available": updates, "state": shell.update_state_word(), "changelog_open": shell.changelog_shown() }, "projects": projects, "theme": { "name": theme.name.as_ref(), "appearance": format!("{:?}", theme.appearance) }, "view": shell.view().words(), "bots": bots })
}

/// One session: who the agent is, how it stands, and the rows its list shows.
pub(super) fn session_json(s: &AgentSession, front: bool) -> Value {
    let rows: Vec<Value> = s.shown.iter().map(|row| row_json(s, *row)).collect();
    json!({
        "key": s.key.as_ref(),
        "agent": s.agent.name,
        "title": s.title.as_ref(),
        "status": format!("{:?}", s.status),
        "working": s.conversation.working(),
        "starting": s.starting,
        "problem": s.problem.as_ref().map(|p| p.as_ref()),
        "front": front,
        "rows": rows,
    })
}

fn row_json(s: &AgentSession, row: Row) -> Value {
    match row {
        Row::Item(ix) => s.conversation.items().get(ix).map_or(json!({ "kind": "missing" }), item_json),
        Row::Activity { from, to } => json!({ "kind": "activity", "from": from, "to": to }),
        Row::Waiting => json!({ "kind": "waiting", "label": s.agent.look.labels.waiting.as_ref() }),
    }
}

fn item_json(item: &Item) -> Value {
    match item {
        Item::User { text } => json!({ "kind": "user", "text": cut(text) }),
        Item::Text { text, .. } => json!({ "kind": "agent_text", "text": cut(text) }),
        Item::Thinking { text, .. } => json!({ "kind": "thinking", "text": cut(text) }),
        Item::Tool(call) => json!({ "kind": "tool", "name": call.call.name }),
        Item::Subagent { .. } => json!({ "kind": "subagent" }),
        Item::Permission { .. } => json!({ "kind": "permission" }),
        Item::Notice(text) => json!({ "kind": "notice", "text": cut(text) }),
    }
}

fn cut(text: &str) -> String {
    text.chars().take(TEXT_KEPT).collect()
}

/// A press the way a hand makes it: the pointer comes onto the element, then it goes down, then it lets go.
/// Each step runs once the one before has been handled, so the element knows it is hovered when the press arrives.
/// No step waits for a frame, so a window that nothing is drawing, as one under another window, takes the press too.
pub(crate) fn press_in_steps(window: &mut Window, position: gpui_kit::Point<gpui_kit::Pixels>, cx: &mut App) {
    press_with(window, position, gpui_kit::MouseButton::Left, cx)
}

/// [`press_in_steps`] with the button named.
fn press_with(window: &mut Window, position: gpui_kit::Point<gpui_kit::Pixels>, button: gpui_kit::MouseButton, cx: &mut App) {
    use gpui_kit::{Modifiers, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};
    let modifiers = Modifiers::default();
    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers }), cx);
    window.defer(cx, move |window, cx| {
        let down = MouseDownEvent { button, position, modifiers, click_count: 1, first_mouse: false };
        window.dispatch_event(PlatformInput::MouseDown(down), cx);
        window.defer(cx, move |window, cx| {
            let up = MouseUpEvent { button, position, modifiers, click_count: 1 };
            window.dispatch_event(PlatformInput::MouseUp(up), cx);
        });
    });
}
