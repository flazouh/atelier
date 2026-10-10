use crate::enums::Voice;

impl Voice {
    /// How a bot with this voice writes, as one line the agent is told.
    pub(in super::super) fn manner(self) -> &'static str {
        match self {
            Voice::CalmAndClear => "calm and clear. Use plain words and short sentences.",
            Voice::Cheerful => "cheerful. Be warm and upbeat, and stay brief.",
            Voice::ShortAndDry => "short and dry. Use few words and no filler.",
            Voice::Thorough => {
                "thorough. Say what you checked and what you found, and leave nothing out."
            }
            Voice::Playful => "playful. Keep it light, with a little humour, and stay exact.",
        }
    }
}
