/// The last component of a working folder: `/home/a/atelier/` is `atelier`.
pub(crate) fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default().to_owned()
}
