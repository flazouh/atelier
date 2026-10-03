use crate::{codex::{BACKEND, Codex, consts::KNOT}, registry, session::PermissionMode};

#[test]
fn codex_runs_the_acp_adapter_and_names_its_modes() {
    let agent = Codex::agent();
    assert_eq!((agent.program.as_str(), agent.args.as_slice()), ("npx", ["-y".to_string(), "@agentclientprotocol/codex-acp".to_string()].as_slice()));
    let modes: Vec<_> = agent.modes.iter().map(|(mode, id)| (*mode, id.as_str())).collect();
    assert_eq!(
        modes,
        [
            (PermissionMode::Ask, "read-only"),
            (PermissionMode::AcceptEdits, "workspace-write"),
            (PermissionMode::Auto, "agent"),
            (PermissionMode::Bypass, "agent-full-access"),
        ]
    );
    assert!(agent.models.iter().all(|model| !model.id.contains('[')), "the effort is the adapter's own option, not part of a model");
}

#[test]
fn codex_is_offered_with_its_mark_and_found_by_its_backend() {
    let codex = registry::by_backend(BACKEND).expect("Codex is offered");
    assert_eq!(codex.name, "Codex");
    assert!(codex.mark.is_some(), "it wears Codex's mark");
    let caps = codex.backend.capabilities();
    assert!(caps.resume && caps.interrupt);
    assert_eq!(caps.permission_modes, [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Auto, PermissionMode::Bypass]);
}

#[test]
fn codex_comes_after_claude_code_and_cursor() {
    let names: Vec<_> = registry::agents().iter().map(|agent| agent.name).collect();
    assert_eq!(names, ["Claude Code", "Cursor", "Codex", "atelier"]);
}

#[test]
fn codexs_look_names_codex_and_its_mark_is_served() {
    let look = Codex::look();
    assert_eq!(look.labels.waiting.as_ref(), "Waiting for Codex…");
    assert_eq!(look.mark.working.path, KNOT.path);
    assert!(crate::assets::strip_bytes(look.mark.working.path).is_some(), "the app can load the mark");
}

#[test]
fn codex_signs_in_with_its_own_login_not_the_adapters() {
    let backend = registry::by_backend(BACKEND).expect("Codex is offered").backend;
    let command = backend.sign_in("default").expect("atelier can sign Codex in");
    assert_eq!((command.program.to_str(), command.args.as_slice()), (Some("codex"), ["login".to_string()].as_slice()));
}
