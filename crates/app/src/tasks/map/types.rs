use atelier_ui::AgentLook;

/// The look an agent wears, by its name. `neutral` is for an agent atelier has no look for.
pub type Looks<'a> = &'a dyn Fn(&str) -> AgentLook;
