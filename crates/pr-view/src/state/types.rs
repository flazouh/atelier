pub(super) const STEPS: &[&str] = &["CREATE TABLE reviewed (
        host TEXT NOT NULL,
        repo TEXT NOT NULL,
        number INTEGER NOT NULL,
        path TEXT NOT NULL,
        version TEXT NOT NULL,
        at INTEGER NOT NULL,
        PRIMARY KEY (host, repo, number, path)
    );",
    // 2: when the reader last opened a pull request, for "unread" in the list.
    "CREATE TABLE opened (
        host TEXT NOT NULL,
        repo TEXT NOT NULL,
        number INTEGER NOT NULL,
        at INTEGER NOT NULL,
        PRIMARY KEY (host, repo, number)
    );"];
