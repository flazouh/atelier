//! A real pull request on GitHub, read through the signed-in `gh`, for QA. The project is an empty
//! repository whose `origin` names the real one, so git fetches the pull request into the cache the way it
//! would for a checkout. Nothing is ever written: use it with `PrConfig::read_only(true)`.

mod helpers;
mod structs;

pub use helpers::parse;
pub use structs::Real;
