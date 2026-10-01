use atelier_forge::{ChangedFile, Check, HeldComment, Pull, PullRef, Remark, Thread};
use serde::{Deserialize, Serialize};

use super::types::{Part, PartKind};

/// Everything read about a pull request. A field is empty until its part arrives; [`PullData::has`] says
/// which have.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PullData {
    pub reference: PullRef,
    pub pull: Option<Pull>,
    pub files: Vec<ChangedFile>,
    pub threads: Vec<Thread>,
    pub remarks: Vec<Remark>,
    pub checks: Vec<Check>,
    /// The commit the reader last reviewed up to, when the forge keeps it.
    pub review_point: Option<String>,
    /// Comments the reader wrote in an open review and has not sent.
    pub held: Vec<HeldComment>,
    /// Seconds since the Unix epoch when the newest part arrived. `0` before any.
    pub fetched_at: u64,
    /// Which parts have arrived.
    pub(super) loaded: Vec<PartKind>,
}

impl PullData {
    pub fn new(reference: PullRef) -> Self {
        Self {
            reference,
            pull: None,
            files: Vec::new(),
            threads: Vec::new(),
            remarks: Vec::new(),
            checks: Vec::new(),
            review_point: None,
            held: Vec::new(),
            fetched_at: 0,
            loaded: Vec::new(),
        }
    }

    /// Whether `part` has arrived, from the network or from the disk.
    pub fn has(&self, part: PartKind) -> bool {
        self.loaded.contains(&part)
    }

    /// Every part has arrived.
    pub fn complete(&self) -> bool {
        PartKind::ALL.iter().all(|p| self.has(*p))
    }

    /// Takes a part in. `now` is the time it arrived. A failure changes nothing here: what was there stays,
    /// and the caller says the part could not be read.
    pub fn apply(&mut self, part: Part, now: u64) {
        let kind = match part {
            Part::Pull(pull) => {
                self.pull = Some(*pull);
                PartKind::Pull
            }
            Part::Files(files) => {
                self.files = files;
                PartKind::Files
            }
            Part::Threads(threads) => {
                self.threads = threads;
                PartKind::Threads
            }
            Part::Remarks(remarks) => {
                self.remarks = remarks;
                PartKind::Remarks
            }
            Part::Checks(checks) => {
                self.checks = checks;
                PartKind::Checks
            }
            Part::ReviewPoint(point) => {
                self.review_point = point;
                PartKind::ReviewPoint
            }
            Part::Held(held) => {
                self.held = held;
                PartKind::Held
            }
            Part::Failed { .. } => return,
        };
        if !self.loaded.contains(&kind) {
            self.loaded.push(kind);
        }
        self.fetched_at = now;
    }

    /// The comments in the threads, unsent ones included.
    pub fn comment_count(&self) -> usize {
        self.threads.iter().map(|t| t.comments.len()).sum::<usize>() + self.remarks.len()
    }
}
