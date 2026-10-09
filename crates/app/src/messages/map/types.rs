use gpui_kit::SharedString;

/// What a message says, and how it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// As typed, for a provider that keeps no formatting.
    Plain(SharedString),
    /// Markdown that [`safe_markdown`](super::safe_markdown) has made safe.
    Markdown(SharedString),
}

/// Where a channel stands in the sidebar of its account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Channels,
    Direct,
}
