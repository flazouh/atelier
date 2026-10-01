use super::*;
use crate::registry;

#[test]
fn cursor_runs_agent_acp_and_names_its_modes() {
    let agent = agent();
    assert_eq!((agent.program.as_str(), agent.args.as_slice()), ("agent", ["acp".to_string()].as_slice()));
    let modes: Vec<_> = agent.modes.iter().map(|(mode, id)| (*mode, id.as_str())).collect();
    assert_eq!(modes, [(PermissionMode::Ask, "agent"), (PermissionMode::Plan, "plan")]);
    assert!(!agent.models.is_empty());
}

#[test]
fn cursor_is_offered_with_its_mark_and_found_by_its_backend() {
    let cursor = registry::by_backend(BACKEND).expect("Cursor is offered");
    assert_eq!(cursor.name, "Cursor");
    assert!(cursor.mark.is_some(), "it wears Cursor's mark");
    assert_eq!(cursor.backend.name(), BACKEND);
    let caps = cursor.backend.capabilities();
    assert!(caps.resume && caps.interrupt);
    assert_eq!(caps.permission_modes, [PermissionMode::Ask, PermissionMode::Plan]);
}

#[test]
fn cursors_look_names_cursor_and_its_mark_is_served() {
    let look = look();
    assert_eq!(look.labels.waiting.as_ref(), "Waiting for Cursor…");
    assert_eq!(look.mark.working.frames, 1, "the cube holds still");
    assert!(crate::assets::strip_bytes(look.mark.working.path).is_some(), "the app can load the cube");
}
