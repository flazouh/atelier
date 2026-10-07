/// Why `name` cannot name a file or a folder, or `None` when it can.
pub(crate) fn bad_name(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("Type a name.")
    } else if name == "." || name == ".." {
        Some("Use another name.")
    } else if name.contains(['/', '\\', '\0']) {
        Some("A name has no slash in it.")
    } else {
        None
    }
}

/// `name` in the folder `parent` (`""` is the project's folder).
pub(crate) fn joined(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") }
}

/// Where `path` goes when `from` moves to `to`: `to` for `from` itself, the same place under `to` for what is inside
/// it, and `None` for anything else. An empty `to` still says whether `path` is `from` or inside it.
pub(crate) fn moved(path: &str, from: &str, to: &str) -> Option<String> {
    if path == from {
        return Some(to.to_string());
    }
    let rest = path.strip_prefix(from)?.strip_prefix('/')?;
    Some(joined(to, rest))
}

/// The name a copy of `path` takes in its own folder: `b copy.txt`, then `b copy 2.txt`, with the first that `taken` does not
/// say is there already.
pub(crate) fn copy_name(taken: impl Fn(&str) -> bool, path: &str) -> String {
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    // A dot at the start (`.gitignore`) is part of the name, not the start of an extension.
    let (stem, extension) = match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        _ => (name, ""),
    };
    (1..)
        .map(|n| if n == 1 { format!("{stem} copy{extension}") } else { format!("{stem} copy {n}{extension}") })
        .map(|copy| joined(parent, &copy))
        .find(|copy| !taken(copy))
        .unwrap_or_else(|| path.to_string())
}
