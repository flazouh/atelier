//! The capabilities the app has, and the one gateway that gives them to agents. Each project that starts an agent
//! registers its tasks here; each Claude Code session asks for a [`Grant`](atelier_gateway::Grant) and starts with
//! `--mcp-config` pointing at it. A gateway that cannot start never stops a session: the session starts without it,
//! and the app logs one line.
mod helpers;
mod structs;

pub(crate) use structs::CapabilityHub;

#[cfg(test)]
mod tests;
