use crate::types::RunFailure;

/// How a call reaches Gmail. [`GmailMail`](crate::GmailMail) hands it the arguments of one `gmailcli` call (always ending
/// with `-json`) and reads the JSON it prints. A runner may run the tool here, over SSH, or in a test.
pub trait Runner: Send + Sync {
    /// Runs one call and gives what it printed on stdout. A call that fails gives a [`RunFailure`].
    fn run(&self, args: &[String]) -> Result<String, RunFailure>;

    /// Whether [`read_file`](Self::read_file) works. Attachment download needs it, because `gmailcli` writes files where it
    /// runs. A provider lists `download_attachment` only when this is true.
    fn reads_files(&self) -> bool {
        false
    }

    /// The bytes of a file that `gmailcli` wrote, on the machine where it ran.
    fn read_file(&self, _path: &str) -> Result<Vec<u8>, RunFailure> {
        Err(RunFailure::Spawn("this runner cannot read files".into()))
    }

    /// Removes a directory that `gmailcli` wrote into. Best effort.
    fn remove_dir(&self, _path: &str) {}

    /// A directory that exists where `gmailcli` runs and may be written.
    fn temp_dir(&self) -> String {
        "/tmp".into()
    }
}
