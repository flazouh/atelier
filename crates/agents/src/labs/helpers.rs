/// The embedded mark at an asset path.
pub(crate) fn bytes(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "labs/anthropic-light.svg" => include_bytes!("../../assets/labs/anthropic-light.svg"),
        "labs/anthropic-dark.svg" => include_bytes!("../../assets/labs/anthropic-dark.svg"),
        "labs/openai-light.svg" => include_bytes!("../../assets/labs/openai-light.svg"),
        "labs/openai-dark.svg" => include_bytes!("../../assets/labs/openai-dark.svg"),
        "labs/openrouter-light.svg" => include_bytes!("../../assets/labs/openrouter-light.svg"),
        "labs/openrouter-dark.svg" => include_bytes!("../../assets/labs/openrouter-dark.svg"),
        "labs/copilot-light.svg" => include_bytes!("../../assets/labs/copilot-light.svg"),
        "labs/copilot-dark.svg" => include_bytes!("../../assets/labs/copilot-dark.svg"),
        _ => return None,
    })
}
