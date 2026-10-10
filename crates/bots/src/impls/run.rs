use crate::consts::BRIEF_MAX;
use crate::enums::{RunState, StepState};
use crate::structs::{BotId, HandOver, Run, StepReading};

impl Run {
    /// Every rule the run breaks, in words. Empty when it is good.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.brief.trim().is_empty() || self.brief.chars().count() > BRIEF_MAX {
            out.push(format!(
                "{}: the brief needs 1 to {BRIEF_MAX} characters",
                self.id
            ));
        }
        if self.steps.is_empty() {
            out.push(format!("{}: a run needs at least one step", self.id));
        }
        let turn = self.current_step().unwrap_or(self.steps.len());
        for (i, step) in self.steps.iter().enumerate() {
            let (id, n, state) = (&self.id, i + 1, &step.state);
            if i > turn && !matches!(state, StepState::Waiting | StepState::Skipped) {
                out.push(format!(
                    "{id}: step {n} is {state}, and step {} before it is not over",
                    turn + 1
                ));
            }
            match state {
                StepState::Waiting if step.session.is_some() => {
                    out.push(format!("{id}: step {n} waits and has a session"));
                }
                StepState::Working | StepState::Done if step.session.is_none() => {
                    out.push(format!("{id}: step {n} is {state} and has no session"));
                }
                StepState::Failed { reason } if reason.trim().is_empty() => {
                    out.push(format!("{id}: step {n} failed with no reason"));
                }
                _ => {}
            }
            if step.hand_over.is_some() && *state != StepState::Done {
                out.push(format!(
                    "{id}: step {n} has a hand-over and is not done"
                ));
            }
        }
        out
    }

    /// Where the run is, read from its steps.
    pub fn state(&self) -> RunState {
        if self.stopped {
            return RunState::Stopped;
        }
        match self.current_step().map(|step| &self.steps[step].state) {
            None => RunState::Done,
            Some(StepState::Failed { .. }) => RunState::Failed,
            Some(StepState::NeedsPerson { .. }) => RunState::NeedsPerson,
            Some(_) => RunState::Working,
        }
    }

    /// The step that has its turn: the first one that is not done and not skipped. `None` when every step is over.
    pub fn current_step(&self) -> Option<usize> {
        self.steps.iter().position(|step| !step.state.is_over())
    }

    /// The step the app can start now: the one that has its turn, when it waits and the run is not stopped.
    pub fn ready_step(&self) -> Option<usize> {
        self.current_step()
            .filter(|step| !self.stopped && self.steps[*step].state == StepState::Waiting)
    }

    /// What the bot of a step must read. `None` when the run has no such step.
    pub fn reading(&self, step: usize) -> Option<StepReading> {
        (step < self.steps.len()).then(|| self.reading_of(step))
    }

    /// The bots the run still needs: the bots of the steps that are not done and not skipped, each one once. A
    /// stopped run needs none.
    pub fn bots_needed(&self) -> Vec<BotId> {
        if self.stopped {
            return Vec::new();
        }
        let mut bots: Vec<BotId> = self
            .steps
            .iter()
            .filter(|step| !step.state.is_over())
            .map(|step| step.bot.clone())
            .collect();
        bots.sort();
        bots.dedup();
        bots
    }

    pub(super) fn reading_of(&self, step: usize) -> StepReading {
        let hand_overs = self.steps[..step]
            .iter()
            .filter_map(|done| {
                let text = done.hand_over.clone()?;
                Some(HandOver {
                    bot: done.bot.clone(),
                    text,
                })
            })
            .collect();
        StepReading {
            brief: self.brief.clone(),
            hand_overs,
            answer: self.steps[step].answer.clone(),
        }
    }
}
