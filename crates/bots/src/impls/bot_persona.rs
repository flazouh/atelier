use crate::structs::Bot;

impl Bot {
    /// Who the bot is, as text for the agent that runs it: its name, role, job, voice and skills. A harness that takes
    /// a system prompt gets this added to its own. It says nothing of the tools the bot may use.
    pub fn persona(&self) -> String {
        let mut lines = vec![
            format!(
                "In this session you are {}, a bot that works for the person you talk to.",
                self.name.trim()
            ),
            format!("Role: {}.", self.role.trim().trim_end_matches('.')),
        ];
        if !self.job.trim().is_empty() {
            lines.push(format!("Job: {}", self.job.trim()));
        }
        lines.push(format!("Voice: {}", self.voice.manner()));
        if !self.skills.is_empty() {
            lines.push(format!(
                "Skills: {}. When a task fits one of these skills and you have it, use it. A name that ends in * stands for every skill whose name starts that way.",
                self.skills.join(", ")
            ));
        }
        lines.push(format!(
            "When you say who you are, you are {}.",
            self.name.trim()
        ));
        lines.join("\n")
    }
}
