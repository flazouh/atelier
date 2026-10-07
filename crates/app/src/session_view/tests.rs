use atelier_ui::{
    agent_panels::AgentPanels,
    panel_types::{PanelData, ProjectLabel},
    session_status::SessionStatus,
    sidebar_model::Location,
};
use gpui_kit::{
    AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, Render, Styled, TestAppContext,
    Window, div, px,
    component::input::{Input, InputState},
};

/// Agent panels holding one panel whose content is an input, as a session's composer is.
struct Host {
    panels: Entity<AgentPanels>,
    input: Entity<InputState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.panels.clone())
    }
}

/// A press on an input inside a panel leaves the focus in the input, so typing goes there and not to
/// the window's bare-letter keys.
#[gpui_kit::test]
fn a_press_on_an_input_in_a_panel_focuses_the_input(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Dark, cx);
        cx.set_reduce_motion(true);
    });
    let (host, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx));
        let panels = cx.new(AgentPanels::new);
        let shown = input.clone();
        let panel = PanelData {
            id: "p".into(),
            project: ProjectLabel { id: "project".into(), name: "project".into(), location: Location::Local, badge: None },
            title: "A session".into(),
            look: atelier_agents::registry::agents()[0].look.clone(),
            status: SessionStatus::Idle,
            content: atelier_ui::panel_types::content_from(move |_, _| {
                div().size_full().child(div().debug_selector(|| "composer".into()).h(px(40.)).child(Input::new(&shown))).into_any_element()
            }, cx),
        };
        panels.update(cx, |p, cx| p.set_panels(vec![panel], vec!["project".into()], cx));
        Host { panels, input }
    });
    cx.run_until_parked();
    let at = cx.debug_bounds("composer").expect("the panel draws its input");
    cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    let focused = cx.update(|window, cx| host.read(cx).input.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "the input kept the focus");
}

/// A9: Stop shows only while a turn goes on.
#[test]
fn stop_shows_only_while_a_turn_runs() {
    use super::shows_stop;
    assert!(shows_stop(true, &SessionStatus::Working));
    assert!(!shows_stop(true, &SessionStatus::Idle));
    assert!(!shows_stop(true, &SessionStatus::Finished));
    assert!(!shows_stop(false, &SessionStatus::Working));
    assert!(!shows_stop(false, &SessionStatus::Failed("gone".into())));
}

#[test]
fn cards_and_flat_rows_stack_close_and_prose_keeps_its_room() {
    use atelier_agents::session::ToolKind;
    assert!(super::is_lookup(ToolKind::Read) && super::is_lookup(ToolKind::Search));
    assert!(!super::is_lookup(ToolKind::Shell) && !super::is_lookup(ToolKind::Edit) && !super::is_lookup(ToolKind::Fetch));
    use super::Block::{Card, Flat, Prose};
    let stack = atelier_ui::STACK_GAP;
    assert_eq!(super::gap_between(Flat, Some(Flat), 14.), 2.);
    assert_eq!(super::gap_between(Flat, Some(Card), 14.), stack);
    assert_eq!(super::gap_between(Card, Some(Flat), 14.), stack);
    assert_eq!(super::gap_between(Card, Some(Card), 14.), stack);
    assert_eq!(super::gap_between(Card, Some(Prose), 14.), 14.);
    assert_eq!(super::gap_between(Prose, Some(Card), 14.), 14.);
    assert_eq!(super::gap_between(Card, None, 14.), 14.);
}

/// After a turn changed a file, the files the session changed show folded above the composer, with
/// Review; the panel's head has no Review of its own. Review opens the whole session's review.
#[gpui_kit::test]
fn the_changed_files_sit_above_the_composer_and_review_opens_the_session(cx: &mut TestAppContext) {
    use std::{cell::RefCell, rc::Rc};
    let dir = crate::fake_agent::git_project(&[("a.txt", "one\n")]);
    let (session, fake, cx) = crate::fake_agent::start_shown_in(cx, dir.clone(), vec![vec![crate::fake_agent::ended()]]);
    cx.simulate_resize(gpui_kit::size(px(600.), px(800.)));
    let heard = Rc::new(RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(&session, move |_, event: &crate::agent_session::SessionEvent, _| {
            if let crate::agent_session::SessionEvent::Review { turn, path } = event {
                log.borrow_mut().push((*turn, path.clone()));
            }
        })
        .detach()
    });
    assert!(cx.debug_bounds("changed-files-review").is_none(), "nothing changed yet");
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "two\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).changed_files().iter().map(|f| f.path.to_string()).collect::<Vec<_>>()), ["a.txt"]);
    assert!(cx.debug_bounds("panel-review").is_none(), "the head has no Review");
    let composer = cx.debug_bounds("prompt-frame").expect("the composer is drawn");
    let review = cx.debug_bounds("changed-files-review").expect("Review is over the composer");
    assert!(review.bottom() <= composer.top(), "the list sits above the composer");
    cx.simulate_click(review.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(heard.borrow().as_slice(), [(None, Some("a.txt".to_string()))]);
}

/// A file the agent changed beyond the project shows in the changed files by its absolute path, so it reads as outside, and a
/// press on it opens the review on it, since the editor holds only the project's files.
#[gpui_kit::test]
fn a_file_changed_beyond_the_project_shows_by_its_path_and_a_press_reviews_it(cx: &mut TestAppContext) {
    use std::{cell::RefCell, rc::Rc};
    use atelier_agents::session::{Event, ToolCall, ToolId, ToolKind, ToolStatus};
    let dir = crate::fake_agent::git_project(&[("a.txt", "one\n")]);
    let far = crate::test_dirs::path().join("far.txt");
    std::fs::write(&far, "one\n").unwrap();
    let (session, _, cx) = crate::fake_agent::start_shown_in(cx, dir.clone(), vec![vec![crate::fake_agent::ended()]]);
    cx.simulate_resize(gpui_kit::size(px(600.), px(800.)));
    let heard = Rc::new(RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(&session, move |_, event: &crate::agent_session::SessionEvent, _| {
            if let crate::agent_session::SessionEvent::Review { path, .. } = event {
                log.borrow_mut().push(("review", path.clone()));
            }
        })
        .detach()
    });
    let project: std::sync::Arc<dyn atelier_project::Project> = std::sync::Arc::new(atelier_project::LocalProject::open(&dir).unwrap());
    let mut tracker = atelier_review::TurnTracker::begin(project.as_ref());
    tracker.observe(project.as_ref(), &Event::ToolStarted(ToolCall {
        id: ToolId::new("t1"),
        name: "Edit".into(),
        kind: ToolKind::Edit,
        input: serde_json::Value::Null,
        file: Some(far.display().to_string()),
        parent: None,
        status: ToolStatus::Running,
    }));
    std::fs::write(&far, "one\ntwo\n").unwrap();
    let turn = tracker.finish(project.as_ref());
    cx.update(|_, cx| session.update(cx, |s, cx| {
        s.reviews.turns.push(turn);
        s.diff_session(cx);
    }));
    cx.run_until_parked();
    let shown = far.display().to_string();
    assert_eq!(cx.update(|_, cx| session.read(cx).changed_files().iter().map(|f| f.path.to_string()).collect::<Vec<_>>()), std::slice::from_ref(&shown));
    let header = cx.debug_bounds("changed-files-toggle").expect("the header is drawn");
    cx.simulate_click(header.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    let name: &'static str = Box::leak(format!("changed-file-{shown}").into_boxed_str());
    let row = cx.debug_bounds(name).expect("the file has a row, named by its absolute path");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(heard.borrow().as_slice(), [("review", Some(shown))], "it opens the review, not the editor");
}

#[test]
fn a_read_shows_no_file_content_and_only_its_error_when_it_fails() {
    use atelier_agents::session::{ToolKind, ToolStatus};
    use super::helpers::shows_output;
    assert!(!shows_output(ToolKind::Read, ToolStatus::Done));
    assert!(!shows_output(ToolKind::Read, ToolStatus::Running));
    assert!(shows_output(ToolKind::Read, ToolStatus::Failed), "the error says why");
    assert!(shows_output(ToolKind::Shell, ToolStatus::Done));
    assert!(shows_output(ToolKind::Search, ToolStatus::Done));
}

/// An edit's diff shows a few rows that do not scroll; pressing it opens the review on that file.
#[gpui_kit::test]
fn pressing_an_edits_diff_opens_the_review_on_its_file(cx: &mut TestAppContext) {
    use std::{cell::RefCell, rc::Rc};
    use atelier_agents::session::{Event, FileEdit, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus};
    let dir = crate::fake_agent::git_project(&[("a.rs", "one\n")]);
    let id = ToolId::new("e1");
    let new = (0..30).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
    let turn = vec![
        Event::ToolStarted(ToolCall { id: id.clone(), name: "Edit".into(), kind: ToolKind::Edit, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Running }),
        Event::ToolEdit { id: id.clone(), edit: FileEdit { path: dir.join("a.rs").to_string_lossy().into(), old: String::new(), new } },
        Event::ToolFinished { id, output: ToolOutput { text: "ok".into(), truncated: false, full_at: None, is_error: false } },
        crate::fake_agent::ended(),
    ];
    let (session, _fake, cx) = crate::fake_agent::start_shown_in(cx, dir, vec![turn]);
    cx.update(|_, cx| cx.set_global(crate::tool_density::ToolDensity::Detailed));
    cx.simulate_resize(gpui_kit::size(px(600.), px(900.)));
    let heard = Rc::new(RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(&session, move |_, event: &crate::agent_session::SessionEvent, _| {
            if let crate::agent_session::SessionEvent::Review { turn, path } = event {
                log.borrow_mut().push((*turn, path.clone()));
            }
        })
        .detach()
    });
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let rows = cx.debug_bounds("diff-rows").expect("the edit shows its rows");
    assert_eq!(f32::from(rows.size.height), atelier_ui::preview_clamp::PREVIEW_ROWS as f32 * atelier_ui::file_diff::ROW_HEIGHT, "a few rows, not a scroller");
    assert!(heard.borrow().is_empty());
    cx.simulate_click(rows.center(), gpui_kit::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(heard.borrow().as_slice(), [(None, Some("a.rs".to_string()))]);
}
