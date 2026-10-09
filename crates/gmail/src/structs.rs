use std::{
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::channel,
    },
    time::Duration,
};

use atelier_capabilities::{
    AuthKind, CapError, CapResult, Limits, Ref, Subscription,
    mail::{
        Account, MailCapabilities, MailEvent, MailFeature, MailOperation, MailProvider, Mailbox,
        Message, RefKind, SearchQuery, SearchSyntax, Thread, ThreadSummary, kind_of, local_id,
    },
    tasks::Page,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    helpers::{
        POLL_QUERY, attachment_at, build_query, label_mailbox, map_failure, remote_command,
        row_summary, sanitize_filename, thread_from_wire, valid_id, watch,
    },
    traits::Runner,
    types::{DEFAULT_LIMIT, POLL_DEFAULT, POLL_THREADS, PROVIDER, RunFailure, VISIBLE_THREADS},
};

/// What `gmailcli whoami -json` prints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct WireAccount {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub unread: u32,
}

/// One line of a search: a thread, as Gmail's list shows it. Gmail's own words for the sender are `from` (the address) and
/// `fromName`. The date is the text Gmail shows, in the language of the account.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Row {
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub from_name: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub attachments: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SearchOut {
    #[serde(default)]
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireMessage {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub from_name: String,
    /// The addresses, joined with a comma.
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub body: String,
    /// The names of the files.
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct WireThread {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub messages: Vec<WireMessage>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct WireLabel {
    pub name: String,
    #[serde(default)]
    pub unread: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct WireAttachments {
    #[serde(default)]
    pub written: Vec<String>,
}

/// Runs `gmailcli` as a process: here, or over SSH on the machine that holds the browser login.
#[derive(Clone, Debug)]
pub struct CliRunner {
    program: String,
    host: Option<String>,
}

impl CliRunner {
    /// Runs `program` (a path or a name on `PATH`) on this machine.
    pub fn local(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            host: None,
        }
    }

    /// Runs `program` on `host` with `ssh`, which must work with no password. The arguments are quoted for the remote shell.
    pub fn over_ssh(host: impl Into<String>, program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            host: Some(host.into()),
        }
    }

    fn command(&self, args: &[String]) -> Command {
        match &self.host {
            Some(host) => {
                let mut command = Command::new("ssh");
                command
                    .args(["-o", "BatchMode=yes", host])
                    .arg(remote_command(&self.program, args));
                command
            }
            None => {
                let mut command = Command::new(&self.program);
                command.args(args);
                command
            }
        }
    }

    fn exec(&self, args: &[String]) -> Result<Vec<u8>, RunFailure> {
        finish(
            &self.program,
            self.command(args).stdin(Stdio::null()).output(),
        )
    }

    /// A plain command on the machine that holds the login (`cat`, `rm`), not `gmailcli`.
    fn plain(&self, program: &str, args: &[&str]) -> Result<Vec<u8>, RunFailure> {
        let output = match &self.host {
            Some(host) => {
                let words: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                Command::new("ssh")
                    .args(["-o", "BatchMode=yes", host])
                    .arg(remote_command(program, &words))
                    .stdin(Stdio::null())
                    .output()
            }
            None => Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .output(),
        };
        finish(program, output)
    }
}

/// The stdout of a run that worked, or why it did not.
fn finish(program: &str, output: std::io::Result<Output>) -> Result<Vec<u8>, RunFailure> {
    let output = output.map_err(|e| RunFailure::Spawn(format!("{program}: {e}")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(RunFailure::Exit {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl Runner for CliRunner {
    fn run(&self, args: &[String]) -> Result<String, RunFailure> {
        self.exec(args)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    fn reads_files(&self) -> bool {
        true
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, RunFailure> {
        self.plain("cat", &[path])
    }

    fn remove_dir(&self, path: &str) {
        // Only a directory that this provider made is removed, whatever the caller passes.
        if path.contains("/atelier-gmail-") && !path.contains("..") {
            let _ = self.plain("rm", &["-rf", "--", path]);
        }
    }

    fn temp_dir(&self) -> String {
        match &self.host {
            Some(_) => "/tmp".into(),
            None => std::env::temp_dir().to_string_lossy().into_owned(),
        }
    }
}

/// The Gmail provider. It reads mail through a [`Runner`] and maps what comes back to the neutral mail types.
///
/// What `gmailcli` gives, and so what this provider loses:
/// - Threads, not messages. A message id is `m:<thread>.<n>`, where `n` counts from 0 in thread order, so it holds only while
///   no message is added before it. The provider does not list `stable_message_ids`.
/// - Dates as Gmail shows them, at the minute, in the account's local time. They are read as UTC, so they may be off by the
///   offset. The text is kept in `headers["date"]`; a date that cannot be read is 0.
/// - No flags per message and no mailboxes of a message. A thread read marks the thread read **in Gmail**, because the tool
///   opens it in the browser. So `read` is true, `starred` is false and `mailboxes` is empty on a message. A search summary
///   has the real unread state of the thread (1 for unread, 0 for read; the count is not known), and a `message_count` of at
///   least 1.
/// - `to` has addresses only. There is no `cc`, no `bcc`, no `reply_to`, no HTML and no headers other than the date.
/// - One page of at most 50 threads. A search cursor is an offset into that page.
pub struct GmailMail {
    address: String,
    runner: Arc<dyn Runner>,
    poll_every: Duration,
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

impl GmailMail {
    /// A provider for `address`. The browser behind the runner should be signed in to that account; [`whoami`](MailProvider::whoami)
    /// checks it.
    pub fn new(address: impl Into<String>, runner: Arc<dyn Runner>) -> Self {
        Self {
            address: address.into(),
            runner,
            poll_every: POLL_DEFAULT,
        }
    }

    /// Asks the runner which account the browser is signed in to, and makes a provider for it. It takes one call.
    pub fn connect(runner: Arc<dyn Runner>) -> CapResult<Self> {
        let provider = Self::new("unknown@gmail.invalid", runner);
        let account: WireAccount = provider.call(&["whoami"])?;
        if account.email.is_empty() {
            return Err(CapError::NotSignedIn);
        }
        Ok(Self {
            address: account.email,
            ..provider
        })
    }

    /// How often `subscribe` reads the inbox. The default is 60 s.
    pub fn with_poll_interval(mut self, every: Duration) -> Self {
        self.poll_every = every;
        self
    }

    fn call<T: DeserializeOwned>(&self, args: &[&str]) -> CapResult<T> {
        let mut words: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        words.push("-json".into());
        let out = self.runner.run(&words).map_err(|f| map_failure(&f))?;
        serde_json::from_str(out.trim()).map_err(|e| CapError::Provider {
            code: "bad_output".into(),
            message: format!("gmailcli printed something that is not the expected JSON: {e}"),
        })
    }

    /// Checks that `reference` is a Gmail reference of this account and of the kind asked for.
    fn own(&self, reference: &Ref, kind: RefKind) -> CapResult<()> {
        if reference.provider == PROVIDER
            && reference.account.eq_ignore_ascii_case(&self.address)
            && kind_of(reference) == Some(kind)
        {
            Ok(())
        } else {
            Err(CapError::invalid("ref"))
        }
    }

    fn read_thread(&self, id: &str) -> CapResult<Thread> {
        if !valid_id(id) {
            return Err(CapError::not_found(format!("thread {id}")));
        }
        let wire: WireThread = self.call(&["thread", id, "-full"])?;
        thread_from_wire(&self.address, id, &wire)
            .ok_or_else(|| CapError::not_found(format!("thread {id}")))
    }
}

impl MailProvider for GmailMail {
    fn provider(&self) -> &str {
        PROVIDER
    }

    fn account(&self) -> &str {
        &self.address
    }

    fn capabilities(&self) -> MailCapabilities {
        let mut operations = vec![
            MailOperation::Mailboxes,
            MailOperation::Search,
            MailOperation::Thread,
            MailOperation::Get,
            MailOperation::Subscribe,
        ];
        if self.runner.reads_files() {
            operations.push(MailOperation::DownloadAttachment);
        }
        MailCapabilities {
            operations,
            features: vec![
                MailFeature::Threads,
                MailFeature::Labels,
                MailFeature::Attachments,
            ],
            search_syntax: SearchSyntax::Gmail,
            limits: Limits {
                page_max: Some(VISIBLE_THREADS as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::BrowserSession],
        }
    }

    fn whoami(&self) -> CapResult<Account> {
        let seen: WireAccount = self.call(&["whoami"])?;
        if seen.email.is_empty() {
            return Err(CapError::NotSignedIn);
        }
        if !seen.email.eq_ignore_ascii_case(&self.address) {
            return Err(CapError::Provider {
                code: "account_mismatch".into(),
                message: format!(
                    "the browser is signed in to {}, not {}",
                    seen.email, self.address
                ),
            });
        }
        Ok(Account {
            reference: Ref {
                capability: "mail".into(),
                provider: PROVIDER.into(),
                account: self.address.clone(),
                id: "account".into(),
            },
            address: self.address.clone(),
            name: None,
        })
    }

    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        let labels: Vec<WireLabel> = self.call(&["labels"])?;
        Ok(labels
            .iter()
            .map(|label| label_mailbox(&self.address, label))
            .collect())
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<ThreadSummary>> {
        let offset = match &query.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        if let Some(mailbox) = &query.mailbox {
            self.own(mailbox, RefKind::Mailbox)?;
        }
        let limit = query
            .limit
            .map_or(DEFAULT_LIMIT, |n| (n as usize).clamp(1, VISIBLE_THREADS));
        // One row more than the page, to know whether a next page exists.
        let want = (offset + limit + 1).min(VISIBLE_THREADS);
        let out: SearchOut =
            self.call(&["search", &build_query(query), "-n", &want.to_string()])?;
        let items = out
            .rows
            .iter()
            .skip(offset)
            .take(limit)
            .map(|row| row_summary(&self.address, row))
            .collect();
        Ok(Page {
            items,
            next_cursor: (out.rows.len() > offset + limit).then(|| (offset + limit).to_string()),
        })
    }

    fn thread(&self, thread: &Ref) -> CapResult<Thread> {
        self.own(thread, RefKind::Thread)?;
        self.read_thread(local_id(thread))
    }

    fn get(&self, message: &Ref) -> CapResult<Message> {
        self.own(message, RefKind::Message)?;
        let not_found = || CapError::not_found(message.to_string());
        let (thread, index) = local_id(message).rsplit_once('.').ok_or_else(not_found)?;
        let index: usize = index.parse().map_err(|_| not_found())?;
        self.read_thread(thread)
            .map_err(|e| match e {
                CapError::NotFound { .. } => not_found(),
                other => other,
            })?
            .messages
            .into_iter()
            .nth(index)
            .ok_or_else(not_found)
    }

    fn download_attachment(&self, attachment: &Ref) -> CapResult<Vec<u8>> {
        if !self.runner.reads_files() {
            return Err(CapError::unsupported("download an attachment"));
        }
        self.own(attachment, RefKind::Attachment)?;
        let not_found = || CapError::not_found(attachment.to_string());
        let (thread_id, flat) = local_id(attachment)
            .rsplit_once('.')
            .ok_or_else(not_found)?;
        let flat: usize = flat.parse().map_err(|_| not_found())?;
        let thread = self.read_thread(thread_id).map_err(|e| match e {
            CapError::NotFound { .. } => not_found(),
            other => other,
        })?;
        let wanted = attachment_at(&thread, flat).ok_or_else(not_found)?;
        let dir = format!(
            "{}/atelier-gmail-{}-{}",
            self.runner.temp_dir().trim_end_matches('/'),
            std::process::id(),
            SCRATCH.fetch_add(1, Ordering::Relaxed)
        );
        let result = (|| {
            let written: WireAttachments =
                self.call(&["attachments", thread_id, "-download", &dir])?;
            let name = sanitize_filename(&wanted);
            let path = written
                .written
                .iter()
                .find(|p| p.rsplit('/').next() == Some(name.as_str()))
                .ok_or_else(not_found)?;
            self.runner.read_file(path).map_err(|f| map_failure(&f))
        })();
        self.runner.remove_dir(&dir);
        result
    }

    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        let baseline: SearchOut =
            self.call(&["search", POLL_QUERY, "-n", &POLL_THREADS.to_string()])?;
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        let runner = self.runner.clone();
        let address = self.address.clone();
        let every = self.poll_every;
        std::thread::Builder::new()
            .name("gmail-poll".into())
            .spawn(move || watch(runner, address, every, baseline.rows, tx, stop))
            .map_err(|e| CapError::Provider {
                code: "poll".into(),
                message: e.to_string(),
            })?;
        Ok(subscription)
    }
}

impl std::fmt::Debug for GmailMail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GmailMail")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}
