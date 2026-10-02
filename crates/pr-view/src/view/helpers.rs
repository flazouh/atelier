/// A job that has been asked for and not read, so the same job is not asked for twice.
pub(super) fn placeholder_job(job: &atelier_forge::JobRef) -> atelier_forge::Job {
    atelier_forge::Job { reference: job.clone(), name: String::new(), status: atelier_forge::CheckStatus::Queued, conclusion: None, run_id: 0, attempt: 1, steps: Vec::new() }
}
