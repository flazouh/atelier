use crate::consts::ASKS_FIRST_QUESTION;
use crate::enums::{BotsError, RunEvent, StepState};
use crate::structs::{BotId, Playbook, Run, RunStep};

fn refused<T>(why: String) -> Result<T, BotsError> {
    Err(BotsError::Refused(why))
}

/// The text with no space around it, or `None` when nothing is left.
fn words(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The moves of a run. Each one leaves the run it is called on as it was, and returns the new run with what
/// happened, or says in words why the move is not allowed. A step is named by its place, from 0. The words of a
/// refusal count from 1, as a person does.
impl Run {
    /// Starts a run of a playbook on a brief. The first step has its turn, and the rest wait.
    pub fn start(
        id: BotId,
        playbook: &Playbook,
        brief: &str,
        now_ms: i64,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = Run::waiting(id, playbook, brief, now_ms)?;
        let event = run.next_turn();
        Ok((run, vec![event]))
    }

    /// Starts a run of one step of a playbook, alone. Every other step is skipped. The step does not ask first:
    /// the person who picked it already said go.
    pub fn start_alone(
        id: BotId,
        playbook: &Playbook,
        step: usize,
        brief: &str,
        now_ms: i64,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = Run::waiting(id, playbook, brief, now_ms)?;
        if step >= run.steps.len() {
            return refused(run.no_step(step));
        }
        for (i, other) in run.steps.iter_mut().enumerate() {
            if i != step {
                other.state = StepState::Skipped;
            }
        }
        let event = run.ready(step);
        Ok((run, vec![event]))
    }

    /// The app started a session for the step that is ready.
    pub fn step_starts(
        &self,
        step: usize,
        session: &str,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let Some(session) = words(session) else {
            return refused(format!(
                "step {} needs the id of its session to start",
                step + 1
            ));
        };
        let state = &run.steps[step].state;
        if *state != StepState::Waiting {
            return refused(format!(
                "step {} is {state}: only a step that waits can start",
                step + 1
            ));
        }
        if let Some(turn) = run.current_step().filter(|turn| *turn != step) {
            return refused(format!(
                "step {} cannot start: step {} is not over",
                step + 1,
                turn + 1
            ));
        }
        run.steps[step].state = StepState::Working;
        run.steps[step].session = Some(session.clone());
        let bot = run.steps[step].bot.clone();
        Ok((run, vec![RunEvent::StepStarted { step, bot, session }]))
    }

    /// A step at work asks a person.
    pub fn step_asks(
        &self,
        step: usize,
        question: &str,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let state = &run.steps[step].state;
        if *state != StepState::Working {
            return refused(format!(
                "step {} is {state}: only a step at work can ask a person",
                step + 1
            ));
        }
        let question = question.trim().to_string();
        run.steps[step].state = StepState::NeedsPerson {
            question: question.clone(),
        };
        let bot = run.steps[step].bot.clone();
        Ok((
            run,
            vec![RunEvent::StepNeedsPerson {
                step,
                bot,
                question,
            }],
        ))
    }

    /// A person answers a step that waits for one. A step at work goes on, and the app gives the answer to its
    /// session. A step that asked before it started is ready, and its bot reads the answer.
    pub fn person_answers(
        &self,
        step: usize,
        answer: &str,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let state = &run.steps[step].state;
        if !matches!(state, StepState::NeedsPerson { .. }) {
            return refused(format!(
                "step {} is {state}: it asked nothing",
                step + 1
            ));
        }
        run.steps[step].answer = words(answer);
        let event = match run.steps[step].session.clone() {
            Some(session) => {
                run.steps[step].state = StepState::Working;
                RunEvent::StepAnswered {
                    step,
                    bot: run.steps[step].bot.clone(),
                    session,
                    answer: answer.trim().to_string(),
                }
            }
            None => {
                run.steps[step].state = StepState::Waiting;
                run.ready(step)
            }
        };
        Ok((run, vec![event]))
    }

    /// A step at work ends, with what its bot wrote for the next steps. The next step that is not skipped has its
    /// turn, and with none left the run is done.
    pub fn step_ends(
        &self,
        step: usize,
        hand_over: &str,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        match &run.steps[step].state {
            StepState::Working => {}
            StepState::Waiting => {
                return refused(format!("step {} never started: it cannot end", step + 1));
            }
            state => {
                return refused(format!(
                    "step {} is {state}: only a step at work can end",
                    step + 1
                ));
            }
        }
        let hand_over = words(hand_over);
        run.steps[step].state = StepState::Done;
        run.steps[step].hand_over = hand_over.clone();
        let done = RunEvent::StepDone {
            step,
            bot: run.steps[step].bot.clone(),
            hand_over,
        };
        let next = run.next_turn();
        Ok((run, vec![done, next]))
    }

    /// The step that has its turn fails, with a reason. It may be ready, at work or waiting for a person.
    pub fn step_fails(
        &self,
        step: usize,
        reason: &str,
    ) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let Some(reason) = words(reason) else {
            return refused(format!("step {} needs a reason to fail", step + 1));
        };
        let state = &run.steps[step].state;
        if state.is_over() || matches!(state, StepState::Failed { .. }) {
            return refused(format!("step {} is {state}: it cannot fail", step + 1));
        }
        if let Some(turn) = run.current_step().filter(|turn| *turn != step) {
            return refused(format!(
                "step {} cannot fail before its turn: step {} is not over",
                step + 1,
                turn + 1
            ));
        }
        run.steps[step].state = StepState::Failed {
            reason: reason.clone(),
        };
        let bot = run.steps[step].bot.clone();
        Ok((run, vec![RunEvent::RunFailed { step, bot, reason }]))
    }

    /// A person skips a step that is not over. It may be a later step, ahead of its turn. When the skipped step
    /// had its turn, the next one gets it.
    pub fn skip_step(&self, step: usize) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let state = &run.steps[step].state;
        if state.is_over() {
            return refused(format!(
                "step {} is {state}: it cannot be skipped",
                step + 1
            ));
        }
        let had_its_turn = run.current_step() == Some(step);
        run.steps[step].state = StepState::Skipped;
        let mut events = vec![RunEvent::StepSkipped {
            step,
            bot: run.steps[step].bot.clone(),
            session: run.steps[step].session.clone(),
        }];
        if had_its_turn {
            events.push(run.next_turn());
        }
        Ok((run, events))
    }

    /// A person tries a failed step again. It is ready at once, for a new session, and it does not ask first again.
    pub fn retry_step(&self, step: usize) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.open(step)?;
        let state = &run.steps[step].state;
        if !matches!(state, StepState::Failed { .. }) {
            return refused(format!(
                "step {} is {state}: only a failed step can be tried again",
                step + 1
            ));
        }
        run.steps[step].state = StepState::Waiting;
        run.steps[step].session = None;
        let event = run.ready(step);
        Ok((run, vec![event]))
    }

    /// A person stops the run. The steps stay as they were, and no move changes the run after that.
    pub fn stop(&self) -> Result<(Run, Vec<RunEvent>), BotsError> {
        let mut run = self.checked()?;
        let Some(turn) = run.current_step() else {
            return refused("the run is done: there is nothing to stop".to_string());
        };
        run.stopped = true;
        let session = run.steps[turn].session.clone();
        Ok((run, vec![RunEvent::RunStopped { session }]))
    }

    /// A run with every step waiting, checked.
    fn waiting(
        id: BotId,
        playbook: &Playbook,
        brief: &str,
        now_ms: i64,
    ) -> Result<Run, BotsError> {
        let run = Run {
            id,
            playbook: playbook.id.clone(),
            brief: brief.trim().to_string(),
            started_ms: now_ms,
            stopped: false,
            steps: playbook
                .steps
                .iter()
                .map(|step| RunStep {
                    bot: step.bot.clone(),
                    asks_first: step.asks_first,
                    state: StepState::Waiting,
                    session: None,
                    hand_over: None,
                    answer: None,
                })
                .collect(),
        };
        let problems = run.problems();
        if problems.is_empty() {
            Ok(run)
        } else {
            Err(BotsError::Invalid(problems))
        }
    }

    /// A copy of the run to move. A run that breaks a rule, and a stopped run, are refused.
    fn checked(&self) -> Result<Run, BotsError> {
        let problems = self.problems();
        if !problems.is_empty() {
            return Err(BotsError::Invalid(problems));
        }
        if self.stopped {
            return refused("the run is stopped: nothing can change it".to_string());
        }
        Ok(self.clone())
    }

    /// A copy of the run to move at this step. A step the run does not have is refused.
    fn open(&self, step: usize) -> Result<Run, BotsError> {
        let run = self.checked()?;
        if step >= run.steps.len() {
            return refused(run.no_step(step));
        }
        Ok(run)
    }

    fn no_step(&self, step: usize) -> String {
        format!(
            "the run has {} steps: there is no step {}",
            self.steps.len(),
            step + 1
        )
    }

    /// Gives the turn to the first step that is not over, and tells what it needs: a person when it asks first,
    /// else a session. With no step left, the run is done.
    fn next_turn(&mut self) -> RunEvent {
        let Some(step) = self.current_step() else {
            return RunEvent::RunDone;
        };
        if !self.steps[step].asks_first {
            return self.ready(step);
        }
        let question = ASKS_FIRST_QUESTION.to_string();
        self.steps[step].state = StepState::NeedsPerson {
            question: question.clone(),
        };
        RunEvent::StepNeedsPerson {
            step,
            bot: self.steps[step].bot.clone(),
            question,
        }
    }

    fn ready(&self, step: usize) -> RunEvent {
        RunEvent::StepReady {
            step,
            bot: self.steps[step].bot.clone(),
            reading: self.reading_of(step),
        }
    }
}
