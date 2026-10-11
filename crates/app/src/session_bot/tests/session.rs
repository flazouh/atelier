use atelier_agents::session::{Command, Event, ToolCall, ToolId, ToolKind, ToolStatus};
use atelier_bot_face::Mood;
use atelier_bots::Bot;
use gpui_kit::TestAppContext;

use crate::fake_agent::{ended, start, start_as};

fn dot() -> Bot {
    atelier_bots::starter_crew().into_iter().find(|b| b.id.as_str() == "dot").unwrap()
}

/// The persona reaches the agent when it is launched, as text added to its system prompt, and never as a message.
#[gpui_kit::test]
fn a_session_of_a_bot_opens_its_agent_with_the_persona_and_sends_none_of_it_as_a_message(cx: &mut TestAppContext) {
    let (session, fake, cx) = start_as(cx, dot(), None, vec![vec![ended()]]);
    assert_eq!(session.read_with(cx, |s, _| s.bot.as_ref().map(|b| b.id.to_string())), Some("dot".to_string()), "the session records its bot");
    let opened = fake.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].append_system_prompt, Some(dot().persona()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("find the leak".into(), cx)));
    cx.run_until_parked();
    let received = fake.received.lock().unwrap();
    let sent: Vec<String> = received.iter().filter_map(|c| match c { Command::Send { text, .. } => Some(text.clone()), _ => None }).collect();
    assert_eq!(sent, ["find the leak"], "the first message is the reader's own words");
}

/// The system prompt is not kept with the agent's session, so a resume says the persona again.
#[gpui_kit::test]
fn a_session_of_a_bot_that_resumes_gets_the_persona_again(cx: &mut TestAppContext) {
    let (_session, fake, _cx) = start_as(cx, dot(), Some("old-1"), vec![]);
    let opened = fake.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].resume.as_ref().map(|id| id.as_str()), Some("old-1"));
    assert_eq!(opened[0].append_system_prompt, Some(dot().persona()));
}

#[gpui_kit::test]
fn a_session_with_no_bot_opens_its_agent_with_no_persona_and_has_no_face(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![], false);
    assert!(session.read_with(cx, |s, _| s.bot.is_none()));
    assert_eq!(fake.opened.lock().unwrap()[0].append_system_prompt, None);
    let face = cx.update(|_, cx| crate::session_bot::row_face(&session, cx));
    assert!(face.is_none());
}

/// What the session does is the mood of its bot's face: idle, thinking once a message went, working while a tool runs,
/// done when the turn ended and nobody looked, idle again once the reader looks.
#[gpui_kit::test]
fn the_mood_follows_what_the_session_does(cx: &mut TestAppContext) {
    let (session, fake, cx) = start_as(cx, dot(), None, vec![]);
    let mood = |cx: &mut gpui_kit::VisualTestContext| session.read_with(cx, |s, _| s.mood());
    // What the agent says next, as it would on its own thread.
    let agent = fake.sinks.lock().unwrap()[0].clone();
    let says = |cx: &mut gpui_kit::VisualTestContext, event: Event| {
        agent(event);
        cx.run_until_parked();
    };
    assert_eq!(mood(cx), Some(Mood::Idle));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("find the leak".into(), cx)));
    cx.run_until_parked();
    assert_eq!(mood(cx), Some(Mood::Thinking), "the agent works and runs no tool");
    let read = ToolCall { id: ToolId::new("t1"), name: "Read".into(), kind: ToolKind::Read, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Running };
    says(cx, Event::ToolStarted(read));
    assert_eq!(mood(cx), Some(Mood::Working), "a tool runs");
    says(cx, ended());
    assert_eq!(mood(cx), Some(Mood::Done), "the turn ended and the reader has not looked");
    cx.update(|_, cx| session.update(cx, |s, cx| s.set_seen(true, cx)));
    assert_eq!(mood(cx), Some(Mood::Idle), "the reader looked");
    let face = cx.update(|_, cx| crate::session_bot::row_face(&session, cx));
    assert!(face.is_some(), "a session of a bot has its face");
}

/// A cached row is drawn again only when it is told: a bot's session says so when its face changes between thinking and
/// working, which its status does not tell. A session with no bot has no face, and says nothing then, as before.
#[gpui_kit::test]
fn a_session_of_a_bot_says_it_changed_when_a_tool_starts_and_one_with_no_bot_does_not(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};
    let read = |id: &str| Event::ToolStarted(ToolCall { id: ToolId::new(id), name: "Read".into(), kind: ToolKind::Read, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Running });
    // How many times the session said it changed when its agent started a tool in the middle of a turn.
    let said_on_a_tool = |session: gpui_kit::Entity<crate::agent_session::AgentSession>, fake: std::sync::Arc<crate::fake_agent::Fake>, cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|_, cx| session.update(cx, |s, cx| s.send("find the leak".into(), cx)));
        cx.run_until_parked();
        let said = Rc::new(Cell::new(0));
        let count = said.clone();
        let _hearing = cx.update(|_, cx| {
            cx.subscribe(&session, move |_, event: &crate::agent_session::SessionEvent, _| {
                if matches!(event, crate::agent_session::SessionEvent::Changed) {
                    count.set(count.get() + 1);
                }
            })
        });
        let agent = fake.sinks.lock().unwrap()[0].clone();
        agent(read("t1"));
        cx.run_until_parked();
        said.get()
    };
    let (session, fake, cx) = start_as(cx, dot(), None, vec![]);
    assert_eq!(said_on_a_tool(session, fake, cx), 1, "thinking became working");
    let (session, fake, cx) = start(cx, vec![], false);
    assert_eq!(said_on_a_tool(session, fake, cx), 0, "no bot, no face: the status is the same");
}

/// A window that shows one face of a session: the one of its row, or the one of its header.
struct Shown {
    session: gpui_kit::Entity<crate::agent_session::AgentSession>,
    header: bool,
}

impl gpui_kit::Render for Shown {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement, Styled};
        let face = if self.header { crate::session_bot::header_face(&self.session, cx) } else { crate::session_bot::row_face(&self.session, cx) };
        gpui_kit::div().size(gpui_kit::px(40.)).children(face)
    }
}

/// The row's face moves only while the session works, and never when the system asks for reduced motion; the header's
/// stands still. A face that moves ticks its runtime each time it is painted; a still one leaves it as it was.
#[gpui_kit::test]
fn the_face_on_a_row_moves_only_while_the_session_works_and_never_under_reduce_motion(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start_as(cx, dot(), None, vec![]);
    let runtime = session.read_with(cx, |s, _| s.bot.as_ref().unwrap().runtime.clone());
    let (shown, cx) = cx.add_window_view(|_, _| Shown { session: session.clone(), header: false });
    // Whether one more frame of the window moved the face.
    let moved = |cx: &mut gpui_kit::VisualTestContext| {
        cx.run_until_parked();
        let before = runtime.borrow().clone();
        shown.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        *runtime.borrow() != before
    };
    assert!(!moved(cx), "an idle session: the face stands still");
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("find the leak".into(), cx)));
    assert!(moved(cx), "the session works: the face moves");
    cx.update(|_, cx| cx.set_reduce_motion(true));
    assert!(!moved(cx), "reduce motion: it stands still though the session works");
    cx.update(|_, cx| cx.set_reduce_motion(false));
    assert!(moved(cx), "and it moves again without it");
    shown.update(cx, |shown, _| shown.header = true);
    assert!(!moved(cx), "the face in the header stands still while the session works");
}
