use std::{
    io::{Read, Write},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use atelier_forge::{Change, Pull, PullRef, RepoRef};
use atelier_project::{Command, Project};

use super::types::{Blob, FETCH_TIMEOUT, GitError, GitResult, LEGACY_DATA, QUICK};
use super::helpers::{
    check_branch, check_sha, is_sha, parse_batch, parse_commits, parse_files, remote_for,
};

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

pub struct PrGit {
    pub(super) project: Arc<dyn Project>,
    /// The folder for caches and checkouts as it was given: absolute, or starting with `~/` or `$HOME/`. Empty:
    /// the `pr-view` folder of the project's data folder when the project has one, else [`LEGACY_DATA`].
    pub(super) given: String,
    /// Where an older atelier kept the cache and the checkouts, moved into the data folder on first use.
    legacy: String,
    /// The same, absolute, once the host has been asked for its home. Asking starts a process, so it waits
    /// for the first use, which is on a background thread.
    data: std::sync::OnceLock<Result<String, GitError>>,
    /// The URL to fetch from, when it is not the one the project's remotes give.
    pub(super) remote: Option<String>,
    /// One thing at a time touches the cache and the checkouts: two views of one repository must not both
    /// make the cache, or both make a checkout.
    busy: std::sync::Mutex<()>,
}

pub(super) struct Ran {
    code: Option<i32>,
    stdout: Vec<u8>,
    pub(super) stderr: String,
}

impl PrGit {
    /// `data` is the folder for caches and checkouts on the project's host: an absolute path, or one that
    /// starts with `~/` or `$HOME/`, which the host's own home replaces.
    pub fn new(project: Arc<dyn Project>, data: &str) -> Self {
        Self { project, given: data.to_string(), legacy: LEGACY_DATA.to_string(), data: std::sync::OnceLock::new(), remote: None, busy: std::sync::Mutex::new(()) }
    }

    /// The folder an older atelier used, in place of [`LEGACY_DATA`]: for a test.
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

    /// Moves the folder an older atelier used to `target`, when there is one and the target is not there yet. One
    /// rename on the host, so the cache and the checkouts come whole. A failure leaves both as they are: the
    /// next open makes a new cache.
    fn move_legacy(&self, target: &str) {
        let script = "old=\"$1\"; case \"$old\" in '~/'*) old=\"$HOME/${old#'~/'}\" ;; '$HOME/'*) old=\"$HOME/${old#'$HOME/'}\" ;; esac; \
                      if [ -d \"$old\" ] && [ ! -e \"$2\" ]; then mkdir -p \"$(dirname \"$2\")\" && mv -- \"$old\" \"$2\"; fi";
        let _ = self.sh(script, &[self.legacy.as_str(), target]);
    }

    /// Runs `command`, writes `input` to it, and reads it to the end. Kills it after `timeout`.
    pub(super) fn run(&self, args: &[&str], input: Option<Vec<u8>>, timeout: Duration) -> GitResult<Ran> {
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

    pub(super) fn text(ran: &Ran) -> String {
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
            let mut specs = vec![format!("+refs/pull/{number}/head:refs/atelier/pr/{number}/head")];
            if !base_known {
                specs.push(format!("+refs/heads/{base_branch}:refs/atelier/base/{base_branch}"));
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
                let name = format!("refs/atelier/base/{base_branch}");
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
                      if [ -d \"$2\" ]; then git --git-dir \"$2\" update-ref -d \"refs/atelier/pr/$3/head\" 2>/dev/null; fi\n\
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
