/// A short name for a bot, a playbook or a run: lower case letters, digits and hyphens. It is the name of its file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BotId(pub(in super::super) String);
