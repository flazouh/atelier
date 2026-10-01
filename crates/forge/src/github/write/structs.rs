/// What a write needs to know about a pull request.
pub(super) struct Target {
    pub(super) id: String,
    /// OPEN, CLOSED or MERGED.
    pub(super) state: String,
    pub(super) head: String,
    pub(super) head_sha: String,
    pub(super) same_repo: bool,
    pub(super) has_queue: bool,
}
