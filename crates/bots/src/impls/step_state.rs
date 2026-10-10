use std::fmt;

use crate::enums::StepState;

impl StepState {
    /// Done or skipped: the step has no more work in this run.
    pub fn is_over(&self) -> bool {
        matches!(self, StepState::Done | StepState::Skipped)
    }
}

impl fmt::Display for StepState {
    /// The state in words that fit after "step 2 is".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StepState::Waiting => "waiting",
            StepState::Working => "at work",
            StepState::NeedsPerson { .. } => "waiting for a person",
            StepState::Done => "done",
            StepState::Failed { .. } => "failed",
            StepState::Skipped => "skipped",
        })
    }
}
