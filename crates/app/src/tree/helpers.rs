pub(super) fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

pub(super) fn name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// Every folder above `path`, so opening them shows it.
pub fn ancestors(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = parent(path);
    while !at.is_empty() {
        out.push(at.to_string());
        at = parent(at);
    }
    out
}
