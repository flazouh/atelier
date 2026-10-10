use crate::structs::{BotId, StepReading};

/// What a move did to a run, for the app to show and to act on. `step` is the place of the step, from 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEvent {
    /// A step can start. The app starts a session of this bot and gives it the reading.
    StepReady {
        step: usize,
        bot: BotId,
        reading: StepReading,
    },
    StepStarted {
        step: usize,
        bot: BotId,
        session: String,
    },
    /// A step waits for a person. The app shows the question as "needs you".
    StepNeedsPerson {
        step: usize,
        bot: BotId,
        question: String,
    },
    /// A person answered a step at work. The app gives the answer to the session.
    StepAnswered {
        step: usize,
        bot: BotId,
        session: String,
        answer: String,
    },
    StepDone {
        step: usize,
        bot: BotId,
        hand_over: Option<String>,
    },
    /// A person skipped a step. The app ends the session, when the step has one.
    StepSkipped {
        step: usize,
        bot: BotId,
        session: Option<String>,
    },
    RunDone,
    RunFailed {
        step: usize,
        bot: BotId,
        reason: String,
    },
    /// A person stopped the run. The app ends the session of the step that had its turn, when it has one.
    RunStopped {
        session: Option<String>,
    },
}
