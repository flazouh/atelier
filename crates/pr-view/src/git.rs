//! The pull request's git, through the project. A pull request's commits, files and file texts come from
//! git, not from the forge: the forge lists the changed files, but only git has the text of both sides. The
//! objects come from a **cache repository** lathe keeps per forge repository, made with `git clone --bare
//! --shared` from the project (so it borrows the project's objects) and fed by `git fetch` of
//! `refs/pull/N/head`. The project's own refs and working tree are never touched. The language server
//! needs real files, so a **checkout** of the head, made with `git archive`, sits beside the cache. Never
//! `git worktree`: a machine may refuse it, and a worktree belongs to the repository it came from.
//!
//! Everything goes through [`Project::spawn`], so a remote project keeps its cache and checkouts on its
//! own host. Arguments are passed as arguments, never joined into a shell line, except in the one script
//! that pipes `git archive` into `tar`, which reads its values from positional parameters.
use std::{
    io::{Read, Write},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use lathe_forge::{Change, Pull, PullRef, RepoRef};
use lathe_project::{Command, Project};

/// How long a fetch may take before it is given up.
const FETCH_TIMEOUT: Duration = Duration::from_secs(900);
const QUICK: Duration = Duration::from_secs(60);
/// A file bigger than this is listed but not diffed.
pub const MAX_TEXT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    /// git ran and said no.
    Failed { what: &'static str, stderr: String },
    /// git could not be started, or was cut off.
    Spawn(String),
    /// The head is not on the forge any more: the branch was deleted or rewritten before it was read.
    HeadGone(String),
    /// A value that is not a commit id or a name git takes.
    Invalid(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed { what, stderr } => write!(f, "git could not {what}: {}", stderr.trim()),
            Self::Spawn(why) => write!(f, "git did not run: {why}"),
            Self::HeadGone(sha) => write!(f, "the pull request's head {} is not on the forge any more", short(sha)),
            Self::Invalid(what) => write!(f, "{what} is not something git can be asked about"),
        }
    }
}

impl std::error::Error for GitError {}

pub type GitResult<T> = Result<T, GitError>;

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// A commit id: 4 to 64 hex digits. Anything else never reaches git as a revision.
pub fn is_sha(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn check_sha(text: &str) -> GitResult<&str> {
    if is_sha(text) { Ok(text) } else { Err(GitError::Invalid(text.chars().take(40).collect())) }
}

/// A branch name safe to put in a ref: no leading dash, no control characters, spaces, `..`, `\`, `~^:?*[`.
fn check_branch(name: &str) -> GitResult<&str> {
    let bad = name.is_empty()
        || name.starts_with(['-', '/'])
        || name.ends_with(['/', '.'])
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name.chars().any(|c| c.is_control() || " \\~^:?*[".contains(c));
    if bad { Err(GitError::Invalid(name.chars().take(60).collect())) } else { Ok(name) }
}

/// One commit of the pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub title: String,
    pub author: String,
    /// Seconds since the Unix epoch.
    pub at: u64,
}

/// A changed file, as git lists it between two commits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    /// Where a renamed or copied file came from.
    pub old_path: Option<String>,
    pub change: Change,
    pub additions: u32,
    pub deletions: u32,
    /// git counts no lines: the file is not text.
    pub binary: bool,
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
}

impl FileEntry {
    /// The file's version for Reviewed State: the blob of the new side, or of the old for a deleted file.
    /// A push that leaves the file as it was leaves this the same.
    pub fn version(&self) -> &str {
        self.new_blob.as_deref().or(self.old_blob.as_deref()).unwrap_or("")
    }
}

/// A file's text on one side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    Text(String),
    Binary,
    /// The side has no such file: a new file's old side.
    Missing,
    TooLarge(u64),
}

impl Blob {
    /// The text, or `""` for a side that has none.
    pub fn text_or_empty(&self) -> &str {
        match self {
            Self::Text(text) => text,
            _ => "",
        }
    }
}

/// A pull request made ready to read: its objects are in the cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub reference: PullRef,
    /// The cache repository, an absolute path on the host.
    pub cache: String,
    pub head: String,
    /// Where the pull request's own changes start.
    pub merge_base: String,
}

/// git for one project.
/// Where an older lathe kept the cache and the checkouts.
pub const LEGACY_DATA: &str = "~/.local/share/lathe/pr";

pub struct PrGit {
    project: Arc<dyn Project>,
    /// The folder for caches and checkouts as it was given: absolute, or starting with `~/` or `$HOME/`. Empty:
    /// the `pr-view` folder of the project's data folder when the project has one, else [`LEGACY_DATA`].
    given: String,
    /// Where an older lathe kept the cache and the checkouts, moved into the data folder on first use.
    legacy: String,
    /// The same, absolute, once the host has been asked for its home. Asking starts a process, so it waits
    /// for the first use, which is on a background thread.
    data: std::sync::OnceLock<Result<String, GitError>>,
    /// The URL to fetch from, when it is not the one the project's remotes give.
    remote: Option<String>,
    /// One thing at a time touches the cache and the checkouts: two views of one repository must not both
    /// make the cache, or both make a checkout.
    busy: std::sync::Mutex<()>,
}

struct Ran {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl PrGit {
    /// `data` is the folder for caches and checkouts on the project's host: an absolute path, or one that
    /// starts with `~/` or `$HOME/`, which the host's own home replaces.
    pub fn new(project: Arc<dyn Project>, data: &str) -> Self {
        Self { project, given: data.to_string(), legacy: LEGACY_DATA.to_string(), data: std::sync::OnceLock::new(), remote: None, busy: std::sync::Mutex::new(()) }
    }

    /// The folder an older lathe used, in place of [`LEGACY_DATA`]: for a test.
    pub fn with_legacy(mut self, folder: impl Into<String>) -> Self {
        self.legacy = folder.into();
        self
    }

    /// Fetches from `url` in place of the project's remotes.
    pub fn with_remote(mut self, url: impl Into<String>) -> Self {
        self.remote = Some(url.into());
        self
    }

    /// The data folder, absolute. Asks the host for its home the first time, when the folder starts with one.
    pub fn data(&self) -> GitResult<String> {
        self.data
            .get_or_init(|| {
                if self.given.is_empty()
                    && let Some(base) = self.project.data_path()
                {
                    let target = format!("{}/pr-view", base.display());
                    self.move_legacy(&target);
                    return Ok(target);
                }
                let given = if self.given.is_empty() { self.legacy.as_str() } else { self.given.as_str() };
                let data = match given.strip_prefix("~/").or_else(|| given.strip_prefix("$HOME/")) {
                    Some(rest) => {
                        let home = self.sh("printf %s \"$HOME\"", &[])?;
                        let home = String::from_utf8_lossy(&home.stdout).trim().to_string();
                        if home.is_empty() {
                            return Err(GitError::Spawn("the host has no home folder".into()));
                        }
                        format!("{}/{rest}", home.trim_end_matches('/'))
                    }
                    None => given.to_string(),
                };
                if data.starts_with('/') { Ok(data) } else { Err(GitError::Invalid(format!("the data folder {data}"))) }
            })
            .clone()
    }

    /// Moves the folder an older lathe used to `target`, when there is one and the target is not there yet. One
    /// rename on the host, so the cache and the checkouts come whole. A failure leaves both as they are: the
    /// next open makes a new cache.
    fn move_legacy(&self, target: &str) {
        let script = "old=\"$1\"; case \"$old\" in '~/'*) old=\"$HOME/${old#'~/'}\" ;; '$HOME/'*) old=\"$HOME/${old#'$HOME/'}\" ;; esac; \
                      if [ -d \"$old\" ] && [ ! -e \"$2\" ]; then mkdir -p \"$(dirname \"$2\")\" && mv -- \"$old\" \"$2\"; fi";
        let _ = self.sh(script, &[self.legacy.as_str(), target]);
    }

    /// Runs `command`, writes `input` to it, and reads it to the end. Kills it after `timeout`.
    fn run(&self, args: &[&str], input: Option<Vec<u8>>, timeout: Duration) -> GitResult<Ran> {
        let command = Command {
            program: "git".into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: None,
            env: vec![("GIT_TERMINAL_PROMPT".into(), "0".into()), ("LC_ALL".into(), "C".into())],
        };
        self.launch(&command, input, timeout)
    }

    /// `sh -c script sh args...`: the values are positional parameters, never part of the script.
    fn sh(&self, script: &str, args: &[&str]) -> GitResult<Ran> {
        let mut all = vec!["-c".to_string(), script.to_string(), "sh".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        let command = Command { program: "sh".into(), args: all, cwd: None, env: Vec::new() };
        self.launch(&command, None, QUICK)
    }

    fn launch(&self, command: &Command, input: Option<Vec<u8>>, timeout: Duration) -> GitResult<Ran> {
        let mut process = self.project.spawn(command).map_err(|e| GitError::Spawn(e.to_string()))?;
        let mut stdin = std::mem::replace(&mut process.stdin, Box::new(std::io::sink()));
        let writer = thread::spawn(move || {
            if let Some(input) = input {
                let _ = stdin.write_all(&input);
            }
            drop(stdin);
        });
        let mut stdout = std::mem::replace(&mut process.stdout, Box::new(std::io::empty()));
        let reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let deadline = Instant::now() + timeout;
        let mut timed_out = false;
        while process.control.running() {
            if Instant::now() > deadline {
                timed_out = true;
                let _ = process.control.kill();
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let code = process.control.wait().ok().flatten();
        let _ = writer.join();
        let stdout = reader.join().unwrap_or_default();
        if timed_out {
            return Err(GitError::Spawn(format!("git took longer than {} s and was stopped", timeout.as_secs())));
        }
        Ok(Ran { code, stdout, stderr: process.control.stderr() })
    }

    /// A git command against a cache repository that must succeed.
    fn cached(&self, cache: &str, what: &'static str, args: &[&str]) -> GitResult<Ran> {
        let mut all = vec!["--git-dir", cache];
        all.extend_from_slice(args);
        let ran = self.run(&all, None, QUICK)?;
        if ran.code == Some(0) { Ok(ran) } else { Err(GitError::Failed { what, stderr: ran.stderr }) }
    }

    fn text(ran: &Ran) -> String {
        String::from_utf8_lossy(&ran.stdout).trim().to_string()
    }

    /// The name of the folder for a repository's cache and checkouts.
    pub fn key(repo: &RepoRef) -> String {
        let raw = format!("{}-{}-{}", repo.host, repo.owner, repo.name);
        raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' }).collect()
    }

    fn cache_path(&self, repo: &RepoRef) -> GitResult<String> {
        Ok(format!("{}/{}/cache.git", self.data()?, Self::key(repo)))
    }

    fn checkout_path(&self, reference: &PullRef) -> GitResult<String> {
        Ok(format!("{}/{}/head-{}", self.data()?, Self::key(&reference.repo), reference.number))
    }

    /// The URL to fetch `repo` from: the one told to [`PrGit::with_remote`], else the project's own remote
    /// that names this repository, else the plain `https` one.
    fn fetch_url(&self, repo: &RepoRef) -> GitResult<String> {
        if let Some(url) = &self.remote {
            return Ok(url.clone());
        }
        let ran = self.run(&["remote", "-v"], None, QUICK)?;
        let listing = String::from_utf8_lossy(&ran.stdout).into_owned();
        Ok(remote_for(&listing, repo).unwrap_or_else(|| format!("https://{}/{}.git", repo.host, repo.slug())))
    }

    /// Makes the cache repository if it is not there.
    fn ensure_cache(&self, repo: &RepoRef) -> GitResult<String> {
        let cache = self.cache_path(repo)?;
        let exists = self.run(&["--git-dir", &cache, "rev-parse", "--is-bare-repository"], None, QUICK)?;
        if exists.code == Some(0) {
            return Ok(cache);
        }
        let root = self.project.root().to_string_lossy().into_owned();
        let parent = format!("{}/{}", self.data()?, Self::key(repo));
        let made = self.sh("mkdir -p \"$1\"", &[&parent])?;
        if made.code != Some(0) {
            return Err(GitError::Failed { what: "make the cache folder", stderr: made.stderr });
        }
        // `--shared` borrows the project's objects, so the cache starts with everything the project has.
        let clone = self.run(&["clone", "--bare", "--shared", "--quiet", "--", &root, &cache], None, FETCH_TIMEOUT)?;
        if clone.code != Some(0) {
            return Err(GitError::Failed { what: "make the cache repository", stderr: clone.stderr });
        }
        Ok(cache)
    }

    fn has_commit(&self, cache: &str, sha: &str) -> bool {
        let spec = format!("{sha}^{{commit}}");
        self.run(&["--git-dir", cache, "cat-file", "-e", &spec], None, QUICK).is_ok_and(|r| r.code == Some(0))
    }

    /// Fetches what a pull request needs into the cache, unless it is there, and finds where its own
    /// changes start.
    pub fn prepare(&self, pull: &Pull) -> GitResult<Prepared> {
        let _one_at_a_time = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        let head = check_sha(&pull.head_sha)?.to_string();
        let base_branch = check_branch(&pull.base)?.to_string();
        let repo = &pull.reference.repo;
        let cache = self.ensure_cache(repo)?;
        let base_sha = if pull.base_sha.is_empty() { None } else { Some(check_sha(&pull.base_sha)?.to_string()) };
        let base_known = base_sha.as_deref().is_some_and(|b| self.has_commit(&cache, b));
        if !self.has_commit(&cache, &head) || !base_known {
            let url = self.fetch_url(repo)?;
            let number = pull.reference.number;
            let mut specs = vec![format!("+refs/pull/{number}/head:refs/lathe/pr/{number}/head")];
            if !base_known {
                specs.push(format!("+refs/heads/{base_branch}:refs/lathe/base/{base_branch}"));
            }
            let mut args = vec!["--git-dir", cache.as_str(), "fetch", "--no-tags", "--quiet", "--", url.as_str()];
            args.extend(specs.iter().map(String::as_str));
            let fetched = self.run(&args, None, FETCH_TIMEOUT)?;
            if fetched.code != Some(0) {
                return Err(GitError::Failed { what: "fetch the pull request", stderr: fetched.stderr });
            }
        }
        if !self.has_commit(&cache, &head) {
            return Err(GitError::HeadGone(head));
        }
        // The base the pull request stands on; failing that, the tip of the base branch as fetched.
        let tip = match base_sha.filter(|b| self.has_commit(&cache, b)) {
            Some(sha) => sha,
            None => {
                let name = format!("refs/lathe/base/{base_branch}");
                Self::text(&self.cached(&cache, "read the base branch", &["rev-parse", "--verify", &name])?)
            }
        };
        let merge_base = Self::text(&self.cached(&cache, "find where the branch starts", &["merge-base", &tip, &head])?);
        Ok(Prepared { reference: pull.reference.clone(), cache, head, merge_base })
    }

    /// The commits after `base` up to the head, newest first.
    pub fn commits(&self, prepared: &Prepared, base: &str) -> GitResult<Vec<Commit>> {
        check_sha(base)?;
        let range = format!("{base}..{}", prepared.head);
        let ran = self.cached(&prepared.cache, "list the commits", &["log", "--no-color", "-z", "--format=%H%x1f%an%x1f%at%x1f%s", &range])?;
        Ok(parse_commits(&String::from_utf8_lossy(&ran.stdout)))
    }

    /// Whether `ancestor` is part of `descendant`'s history.
    pub fn is_ancestor(&self, prepared: &Prepared, ancestor: &str, descendant: &str) -> bool {
        is_sha(ancestor)
            && is_sha(descendant)
            && self.run(&["--git-dir", &prepared.cache, "merge-base", "--is-ancestor", ancestor, descendant], None, QUICK).is_ok_and(|r| r.code == Some(0))
    }

    /// Whether the cache has the commit, for a review point that a force push removed.
    pub fn knows(&self, prepared: &Prepared, sha: &str) -> bool {
        is_sha(sha) && self.has_commit(&prepared.cache, sha)
    }

    /// The files that changed between `base` and the head, with renames found.
    pub fn files(&self, prepared: &Prepared, base: &str) -> GitResult<Vec<FileEntry>> {
        check_sha(base)?;
        let head = prepared.head.as_str();
        let raw = self.cached(&prepared.cache, "list the changed files", &["diff", "--no-color", "--raw", "--no-abbrev", "-z", "-M", base, head, "--"])?;
        let stat = self.cached(&prepared.cache, "count the changed lines", &["diff", "--no-color", "--numstat", "-z", "-M", base, head, "--"])?;
        Ok(parse_files(&raw.stdout, &stat.stdout))
    }

    /// The texts of these blobs, in one process. A blob is `None` for a side that has none.
    pub fn blobs(&self, prepared: &Prepared, shas: &[Option<&str>]) -> GitResult<Vec<Blob>> {
        let mut input = String::new();
        for sha in shas.iter().flatten() {
            check_sha(sha)?;
            input.push_str(sha);
            input.push('\n');
        }
        let ran = self.run(&["--git-dir", &prepared.cache, "cat-file", "--batch"], Some(input.into_bytes()), QUICK)?;
        if ran.code != Some(0) {
            return Err(GitError::Failed { what: "read a file", stderr: ran.stderr });
        }
        let mut answers = parse_batch(&ran.stdout).into_iter();
        Ok(shas.iter().map(|sha| if sha.is_some() { answers.next().unwrap_or(Blob::Missing) } else { Blob::Missing }).collect())
    }

    /// A file of the head that the pull request did not change, for reading Brought In.
    pub fn head_file(&self, prepared: &Prepared, path: &str) -> GitResult<Blob> {
        if path.starts_with('-') || path.contains(['\n', '\0']) {
            return Err(GitError::Invalid(path.chars().take(60).collect()));
        }
        let spec = format!("{}:{path}", prepared.head);
        let ran = self.run(&["--git-dir", &prepared.cache, "cat-file", "--batch"], Some(format!("{spec}\n").into_bytes()), QUICK)?;
        Ok(parse_batch(&ran.stdout).into_iter().next().unwrap_or(Blob::Missing))
    }

    /// Every file of the head, for Go to file.
    pub fn head_files(&self, prepared: &Prepared) -> GitResult<Vec<String>> {
        let ran = self.cached(&prepared.cache, "list the head's files", &["ls-tree", "-r", "-z", "--name-only", &prepared.head])?;
        Ok(String::from_utf8_lossy(&ran.stdout).split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect())
    }

    /// The head as files on the host, for the language server. Made with `git archive` into a folder of
    /// the cache's own; kept while the head stays the same and made again when it moves. Returns the folder.
    pub fn checkout(&self, prepared: &Prepared) -> GitResult<String> {
        let _one_at_a_time = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        let dir = self.checkout_path(&prepared.reference)?;
        let marker = format!("{dir}.sha");
        let script = "if [ \"$(cat \"$2\" 2>/dev/null)\" = \"$3\" ] && [ -d \"$1\" ]; then exit 0; fi\n\
                      set -e\n\
                      rm -rf \"$1\" \"$2\"\n\
                      mkdir -p \"$1\"\n\
                      git --git-dir \"$4\" archive \"$3\" | tar -x -C \"$1\"\n\
                      printf %s \"$3\" > \"$2\"";
        let made = self.launch_sh(script, &[&dir, &marker, &prepared.head, &prepared.cache])?;
        if made.code != Some(0) {
            return Err(GitError::Failed { what: "check out the head", stderr: made.stderr });
        }
        Ok(dir)
    }

    fn launch_sh(&self, script: &str, args: &[&str]) -> GitResult<Ran> {
        let mut all = vec!["-c".to_string(), script.to_string(), "sh".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        let command = Command { program: "sh".into(), args: all, cwd: None, env: vec![("LC_ALL".into(), "C".into())] };
        self.launch(&command, None, FETCH_TIMEOUT)
    }

    /// Forgets a pull request: its checkout and the refs it fetched. The cache repository stays for the
    /// next one.
    pub fn remove(&self, reference: &PullRef) -> GitResult<()> {
        let _one_at_a_time = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        let dir = self.checkout_path(reference)?;
        let cache = self.cache_path(&reference.repo)?;
        let number = reference.number.to_string();
        let script = "rm -rf \"$1\" \"$1.sha\"\n\
                      if [ -d \"$2\" ]; then git --git-dir \"$2\" update-ref -d \"refs/lathe/pr/$3/head\" 2>/dev/null; fi\n\
                      exit 0";
        self.sh(script, &[&dir, &cache, &number])?;
        Ok(())
    }

    /// Removes the checkouts of pull requests not in `keep`: what a cleanup after a closed pull request
    /// does. Only folders named `head-<number>` under this repository's folder are touched.
    pub fn sweep(&self, repo: &RepoRef, keep: &[u64]) -> GitResult<Vec<u64>> {
        let folder = format!("{}/{}", self.data()?, Self::key(repo));
        let listing = self.sh("ls -1 \"$1\" 2>/dev/null", &[&folder])?;
        let mut gone = Vec::new();
        for name in String::from_utf8_lossy(&listing.stdout).lines() {
            let Some(number) = name.strip_prefix("head-").filter(|n| !n.ends_with(".sha")).and_then(|n| n.parse::<u64>().ok()) else { continue };
            if !keep.contains(&number) {
                self.remove(&PullRef { repo: repo.clone(), number })?;
                gone.push(number);
            }
        }
        gone.sort_unstable();
        Ok(gone)
    }
}

/// The URL of the remote in a `git remote -v` listing that names `repo`.
pub fn remote_for(listing: &str, repo: &RepoRef) -> Option<String> {
    let mut found: Option<String> = None;
    for line in listing.lines() {
        let mut parts = line.split_whitespace();
        let (Some(_name), Some(url), Some(kind)) = (parts.next(), parts.next(), parts.next()) else { continue };
        if kind == "(fetch)" && RepoRef::from_remote(url).as_ref() == Some(repo) {
            // A remote called `origin` wins over the others that name the same repository.
            if line.starts_with("origin") || found.is_none() {
                found = Some(url.to_string());
            }
        }
    }
    found
}

/// `git log -z --format=%H%x1f%an%x1f%at%x1f%s` as commits.
pub fn parse_commits(output: &str) -> Vec<Commit> {
    output
        .split('\0')
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            let mut fields = record.split('\u{1f}');
            let (sha, author, at, title) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
            is_sha(sha).then(|| Commit { sha: sha.into(), title: title.into(), author: author.into(), at: at.parse().unwrap_or(0) })
        })
        .collect()
}

fn change_of(letter: char) -> Change {
    match letter {
        'A' => Change::Added,
        'D' => Change::Deleted,
        'R' => Change::Renamed,
        'C' => Change::Copied,
        _ => Change::Modified,
    }
}

/// `git diff --raw -z` joined to `git diff --numstat -z`, both with `-M`, so they list the same files in
/// the same order.
pub fn parse_files(raw: &[u8], numstat: &[u8]) -> Vec<FileEntry> {
    let raw = String::from_utf8_lossy(raw);
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    let mut entries: Vec<FileEntry> = Vec::new();
    while let Some(meta) = fields.next() {
        let Some(meta) = meta.strip_prefix(':') else { continue };
        let mut parts = meta.split_whitespace();
        let (Some(_old_mode), Some(new_mode), Some(old_sha), Some(new_sha), Some(status)) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { continue };
        let letter = status.chars().next().unwrap_or('M');
        let (old_path, path) = if matches!(letter, 'R' | 'C') {
            let (Some(old), Some(new)) = (fields.next(), fields.next()) else { break };
            (Some(old.to_string()), new.to_string())
        } else {
            let Some(path) = fields.next() else { break };
            (None, path.to_string())
        };
        let zero = |sha: &str| sha.bytes().all(|b| b == b'0');
        entries.push(FileEntry {
            path,
            old_path,
            change: change_of(letter),
            additions: 0,
            deletions: 0,
            // A submodule is a commit id, not text.
            binary: new_mode == "160000",
            old_blob: (!zero(old_sha)).then(|| old_sha.to_string()),
            new_blob: (!zero(new_sha)).then(|| new_sha.to_string()),
        });
    }
    let stat = String::from_utf8_lossy(numstat);
    let mut parts = stat.split('\0').filter(|f| !f.is_empty()).peekable();
    let mut at = 0;
    while let Some(record) = parts.next() {
        let mut cells = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(name)) = (cells.next(), cells.next(), cells.next()) else { continue };
        // A rename has an empty name here, and the two names follow.
        if name.is_empty() {
            parts.next();
            parts.next();
        }
        if let Some(entry) = entries.get_mut(at) {
            match (added.parse::<u32>(), deleted.parse::<u32>()) {
                (Ok(a), Ok(d)) => {
                    entry.additions = a;
                    entry.deletions = d;
                }
                _ => entry.binary = true,
            }
        }
        at += 1;
    }
    entries
}

/// `git cat-file --batch` output: `<name> blob <size>\n<bytes>\n` for a hit, `<name> missing\n` for a miss.
pub fn parse_batch(output: &[u8]) -> Vec<Blob> {
    let mut blobs = Vec::new();
    let mut at = 0;
    while at < output.len() {
        let Some(end) = output[at..].iter().position(|b| *b == b'\n') else { break };
        let header = String::from_utf8_lossy(&output[at..at + end]).into_owned();
        at += end + 1;
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            blobs.push(Blob::Missing);
            continue;
        }
        // The name comes first and may hold spaces, so the kind and the size are read from the end.
        let mut fields = header.rsplitn(3, ' ');
        let (Some(size), Some(kind), Some(_name)) = (fields.next(), fields.next(), fields.next()) else { break };
        let Ok(size) = size.parse::<usize>() else { break };
        if at + size > output.len() {
            break;
        }
        let body = &output[at..at + size];
        at += size + 1;
        blobs.push(if kind != "blob" {
            Blob::Binary
        } else if size as u64 > MAX_TEXT {
            Blob::TooLarge(size as u64)
        } else if body.iter().take(8000).any(|b| *b == 0) {
            Blob::Binary
        } else {
            match String::from_utf8(body.to_vec()) {
                Ok(text) => Blob::Text(text),
                Err(_) => Blob::Binary,
            }
        });
    }
    blobs
}
