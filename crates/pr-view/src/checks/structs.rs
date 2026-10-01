use atelier_forge::Job;

/// Everything read about a job: its steps and its log.
#[derive(Clone, Debug, PartialEq)]
pub struct JobLog {
    pub job: Job,
    pub log: String,
}
