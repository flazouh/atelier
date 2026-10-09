//! What the tool sets share: reading the arguments of a call, choosing the account, cutting long text, and marking text
//! that other people wrote. The `tasks`, `messaging` and `mail` sets all call into it.
mod helpers;
mod structs;
#[cfg(test)]
mod tests;

pub(crate) use helpers::{
    account, choose, clip, cut, failed, heading, limit, neutral, optional, pick, required, shorten,
    untrusted, utc, word,
};
