/// How a bot feels. It sets the eyes, the pose and how fast the habits run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Idle,
    Thinking,
    Working,
    Done,
    Needs,
    Stuck,
}
