/// How many paths one `git hash-object` is given.
pub(super) const HASH_BATCH: usize = 200;

/// How many paths one `git cat-file --batch` is given. The names go in before any answer is read, so
/// they must fit in the pipe.
pub(super) const HEAD_BATCH: usize = 500;
