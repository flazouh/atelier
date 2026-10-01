use atelier_forge::{PullRef, RepoRef};

/// `owner/name#number`, on github.com.
pub fn parse(spec: &str) -> Result<PullRef, String> {
    let (slug, number) = spec.split_once('#').ok_or("write it as owner/name#number")?;
    let (owner, name) = slug.split_once('/').ok_or("write it as owner/name#number")?;
    let number = number.parse::<u64>().map_err(|_| "the number after # is not a number".to_string())?;
    Ok(PullRef { repo: RepoRef::new("github.com", owner, name), number })
}
