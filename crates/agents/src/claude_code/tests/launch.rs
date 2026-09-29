use crate::{
    claude_code::launch::command,
    session::{OpenRequest, PermissionMode, SessionId},
};

fn args(request: &OpenRequest) -> Vec<String> {
    command("claude", request).args
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|pair| pair[0] == flag && pair[1] == value)
}

#[test]
fn a_new_session_asks_for_stream_json_and_permission_questions_over_stdio() {
    let args = args(&OpenRequest::default());
    assert!(has_pair(&args, "--input-format", "stream-json"));
    assert!(has_pair(&args, "--output-format", "stream-json"));
    assert!(has_pair(&args, "--permission-prompt-tool", "stdio"));
    assert!(args.contains(&"--include-partial-messages".to_string()));
    assert!(!args.contains(&"--resume".to_string()) && !args.contains(&"--model".to_string()));
    assert!(!args.contains(&"--permission-mode".to_string()), "Ask is claude's own default");
}

#[test]
fn resume_model_and_mode_become_flags() {
    let request = OpenRequest {
        resume: Some(SessionId::new("abc-123")),
        model: Some("sonnet".into()),
        mode: Some(PermissionMode::Plan),
    };
    let args = args(&request);
    assert!(has_pair(&args, "--resume", "abc-123"));
    assert!(has_pair(&args, "--model", "sonnet"));
    assert!(has_pair(&args, "--permission-mode", "plan"));
}

#[test]
fn the_program_is_the_one_given() {
    assert_eq!(command("/opt/claude", &OpenRequest::default()).program, std::path::PathBuf::from("/opt/claude"));
}
