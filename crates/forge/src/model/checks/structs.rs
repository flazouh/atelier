use super::super::repository::RepoRef;
use super::types::{CheckStatus, Conclusion};

/// The run a check belongs to, when it has one.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunInfo {
    pub id: u64,
    /// The workflow's name.
    pub workflow: String,
    pub number: u64,
    pub event: String,
    /// How the run's check suite ended. A job that failed in a suite that succeeded was allowed to fail.
    #[serde(default)]
    pub suite: Option<Conclusion>,
}

/// Which job's steps and log to fetch.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobRef {
    pub repo: RepoRef,
    pub id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub name: String,
    pub status: CheckStatus,
    pub conclusion: Option<Conclusion>,
    pub url: Option<String>,
    /// A rule of the repository asks for it to pass.
    pub required: bool,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    /// Set for a job of a workflow run: its steps and log can be fetched.
    pub job: Option<JobRef>,
    pub run: Option<RunInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Step {
    pub number: u32,
    pub name: String,
    pub status: CheckStatus,
    pub conclusion: Option<Conclusion>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub reference: JobRef,
    pub name: String,
    pub status: CheckStatus,
    pub conclusion: Option<Conclusion>,
    pub run_id: u64,
    /// Which re-run of the run this job belongs to, from 1.
    pub attempt: u32,
    pub steps: Vec<Step>,
}
