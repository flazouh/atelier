use super::HandOver;

/// What the bot of a step must read before it works: the brief, the hand-overs of the steps done before it, in
/// order, and what a person last told this step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepReading {
    pub brief: String,
    pub hand_overs: Vec<HandOver>,
    pub answer: Option<String>,
}
