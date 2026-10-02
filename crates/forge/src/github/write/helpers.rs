use crate::{MergeMethod, Side, Verdict};

/// A branch name for a URL path: each segment percent-encoded, the slashes between them kept. A name may hold
/// `#`, `%`, `?` or spaces, none of which may stand in a path as they are.
pub(in super::super) fn encode_ref(name: &str) -> String {
    name.split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn method_name(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "MERGE",
        MergeMethod::Squash => "SQUASH",
        MergeMethod::Rebase => "REBASE",
    }
}

pub(super) fn event_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Comment => "COMMENT",
        Verdict::Approve => "APPROVE",
        Verdict::RequestChanges => "REQUEST_CHANGES",
    }
}

pub(super) fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "LEFT",
        Side::Right => "RIGHT",
    }
}
