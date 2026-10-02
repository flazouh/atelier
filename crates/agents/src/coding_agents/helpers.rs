pub(crate) fn bytes(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "coding/claude-light.svg" => include_bytes!("../../assets/coding/claude-light.svg"),
        "coding/claude-dark.svg" => include_bytes!("../../assets/coding/claude-dark.svg"),
        "coding/codex-light.svg" => include_bytes!("../../assets/coding/codex-light.svg"),
        "coding/codex-dark.svg" => include_bytes!("../../assets/coding/codex-dark.svg"),
        "coding/cursor-light.svg" => include_bytes!("../../assets/coding/cursor-light.svg"),
        "coding/cursor-dark.svg" => include_bytes!("../../assets/coding/cursor-dark.svg"),
        "coding/grok-light.svg" => include_bytes!("../../assets/coding/grok-light.svg"),
        "coding/grok-dark.svg" => include_bytes!("../../assets/coding/grok-dark.svg"),
        "coding/opencode-light.svg" => include_bytes!("../../assets/coding/opencode-light.svg"),
        "coding/opencode-dark.svg" => include_bytes!("../../assets/coding/opencode-dark.svg"),
        _ => return None,
    })
}
