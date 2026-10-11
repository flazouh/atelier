//! How the sidebar sorts the stories: six groups, so the list starts short and a reader opens what they need.
use atelier_ui::{NavGroup, NavRow, fuzzy};

use super::Story;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Foundations,
    Controls,
    Agent,
    Code,
    PullRequests,
    App,
}

impl Group {
    pub(super) const ALL: [Group; 6] = [Group::Foundations, Group::Controls, Group::Agent, Group::Code, Group::PullRequests, Group::App];

    pub(super) fn name(self) -> &'static str {
        match self {
            Group::Foundations => "Foundations",
            Group::Controls => "Controls",
            Group::Agent => "Agent",
            Group::Code => "Code",
            Group::PullRequests => "Pull requests",
            Group::App => "App",
        }
    }

    pub(super) fn named(name: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|group| group.name() == name)
    }

    /// Its stories, in the order of the stories' own list.
    pub(super) fn stories(self) -> impl Iterator<Item = Story> {
        Story::ALL.into_iter().filter(move |story| story.group() == self)
    }
}

impl Story {
    pub(super) fn group(self) -> Group {
        match self {
            Story::Colors | Story::Typography | Story::Icons | Story::Motion | Story::Spark => Group::Foundations,
            Story::Buttons | Story::Badges | Story::Select | Story::Prompt | Story::Voice | Story::ModelBadge => Group::Controls,
            Story::AgentPanel
            | Story::AgentPanels
            | Story::AgentSidebar
            | Story::AgentReplay
            | Story::Messages
            | Story::Tools
            | Story::Plan
            | Story::Streaming
            | Story::SubagentCard
            | Story::SubagentStrip
            | Story::Variants
            | Story::Bots => Group::Agent,
            Story::Editor | Story::Diffs | Story::Inline | Story::Review | Story::ChangedFiles | Story::Load | Story::Worktrees => Group::Code,
            Story::PullRequest | Story::PullRequests | Story::PullRequestView | Story::PrCard | Story::PrChip | Story::PrChipCards | Story::Merge => Group::PullRequests,
            Story::ChangelogSheet | Story::UsageDashboard | Story::Providers | Story::ProviderSettings | Story::SignInNotice | Story::Tasks => Group::App,
        }
    }

    pub(super) fn titled(title: &str) -> Option<Story> {
        Story::ALL.into_iter().find(|story| story.title() == title)
    }
}

/// The stories that match a few typed letters, the best first.
pub(super) fn found(query: &str) -> Vec<Story> {
    let titles = Story::ALL.map(Story::title);
    fuzzy::rank(query, titles, Story::ALL.len()).into_iter().map(|at| Story::ALL[at]).collect()
}

/// What the sidebar lists. With nothing typed: the groups, each folded unless it is in `open`. With letters
/// typed: the stories that match, flat, each with its group's name as a note.
pub(super) fn listed(query: &str, open: &[Group]) -> Vec<NavGroup> {
    if query.trim().is_empty() {
        return Group::ALL
            .into_iter()
            .map(|group| NavGroup::new(group.name(), group.stories().map(|story| NavRow::new(story.title(), story.title()))).open(open.contains(&group)))
            .collect();
    }
    vec![NavGroup::flat(found(query.trim()).into_iter().map(|story| NavRow::new(story.title(), story.title()).note(story.group().name())))]
}
