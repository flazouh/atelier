/// Where a command comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandSource {
    /// The agent's own: sent to it as `/name args`.
    Agent,
    /// Atelier runs it.
    Atelier,
    /// A skill or a command file of the project: sent to the agent.
    Skill,
}
