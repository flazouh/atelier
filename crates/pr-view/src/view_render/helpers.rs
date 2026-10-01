use atelier_forge::PullState;

/// Whether a description is long enough to be clipped at [`BODY_CLIP`], so it needs the "Show the whole description"
/// button: about seven lines of a rail's width, or more than six lines of its own.
pub(crate) fn body_needs_fold(body: &str) -> bool {
    let lines: usize = body.lines().map(|l| l.chars().count().div_ceil(48).max(1)).sum();
    lines > 6
}

pub(super) fn state_word(state: PullState) -> (&'static str, bool) {
    match state {
        PullState::Open => ("Open", false),
        PullState::Draft => ("Draft", false),
        PullState::Merged => ("Merged", false),
        PullState::Closed => ("Closed", true),
    }
}
