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
            project: ProjectLabel { id: "project".into(), name: "project".into(), location: Location::Local },
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
