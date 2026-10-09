use crate::enums::Mood;

impl Mood {
    /// Every mood, in the order the data and the frame use.
    pub const ALL: [Mood; 6] = [Mood::Idle, Mood::Thinking, Mood::Working, Mood::Done, Mood::Needs, Mood::Stuck];

    /// The position of the mood in `Mood::ALL`.
    pub fn index(self) -> usize {
        match self {
            Mood::Idle => 0,
            Mood::Thinking => 1,
            Mood::Working => 2,
            Mood::Done => 3,
            Mood::Needs => 4,
            Mood::Stuck => 5,
        }
    }

    /// The name the data uses.
    pub fn name(self) -> &'static str {
        match self {
            Mood::Idle => "idle",
            Mood::Thinking => "thinking",
            Mood::Working => "working",
            Mood::Done => "done",
            Mood::Needs => "needs",
            Mood::Stuck => "stuck",
        }
    }
}
