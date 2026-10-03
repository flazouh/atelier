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
        Request::Click { name, x, y } => {
            let at = match (name, x, y) {
                (Some(name), _, _) => super::marks::find(&name, cx).map(|b| b.center()),
                (None, Some(x), Some(y)) => Some(gpui_kit::point(gpui_kit::px(x), gpui_kit::px(y))),
                _ => None,
            };
            match at {
                Some(position) => {
                    press(window, position, cx);
                    json!({ "ok": true, "x": f32::from(position.x), "y": f32::from(position.y) })
                }
                None => json!({ "error": "click needs a name that has been drawn, or x and y" }),
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

/// The projects and sessions open, as a person would list them.
fn state(shell: &Shell, cx: &App) -> Value {
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
            json!({ "name": project.name(), "sessions": sessions })
        })
        .collect();
    json!({ "projects": projects })
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
        Row::Changes { turn } => json!({ "kind": "changes", "turn": turn }),
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

/// A left press and release at `position`, as the pointer would do.
fn press(window: &mut Window, position: gpui_kit::Point<gpui_kit::Pixels>, cx: &mut App) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};
    let modifiers = Modifiers::default();
    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers }), cx);
    window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position, modifiers, click_count: 1, first_mouse: false }), cx);
    window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position, modifiers, click_count: 1 }), cx);
}
