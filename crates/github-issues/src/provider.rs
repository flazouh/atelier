//! [`GithubIssues`]: the issues of one repository as a `tasks` provider.
mod structs;
mod subscribe;

pub use structs::GithubIssues;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
