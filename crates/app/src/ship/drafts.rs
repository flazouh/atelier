//! What the agent is asked to draft (a commit message, a branch name), and what is made of its answer.
//! The agent sees the kept change as a diff; its answer is cleaned of fences and quotes, and a branch
//! name is made one git takes. Pure.

mod helpers;
mod types;

pub use helpers::{branch_name, branch_prompt, commit_prompt, kept_diff, message, with_refs};

#[cfg(test)]
mod tests;
