//! The gateway: one MCP server that lets the agents inside atelier use what the app can do. A capability joins by
//! handing the gateway a [`ToolSet`], a list of tool definitions and one function that runs them. The sets are
//! [`TasksTools`], [`MessagingTools`] and [`MailTools`], over the capabilities of the same names. A set lists only the
//! tools that some connected account can do.
//!
//! - [`Gateway`]: the server. It listens on 127.0.0.1, on a port the system picks, and stops when it is dropped.
//! - [`SessionAccess`]: the address and the bearer token of one agent session. Whatever the session writes is
//!   attributed to the [`Actor`](atelier_capabilities::Actor) the token was issued for.
//! - [`McpConfig`]: the file `claude --mcp-config` reads, private to the user and removed when the session ends.
//! - [`Grant`]: a session token and its config file together, which a session holds for as long as it lives.
//!
//! The server speaks the Streamable HTTP transport of MCP (revision 2025-11-25) without state: every POST gets one
//! JSON answer, and there is no event stream. The crate has no UI and no network beyond the loopback.
mod config;
mod grant;
mod http;
mod mail;
mod messaging;
mod protocol;
mod server;
mod session;
mod shared;
mod tasks;
mod tools;

pub use config::McpConfig;
pub use grant::Grant;
pub use mail::MailTools;
pub use messaging::MessagingTools;
pub use server::Gateway;
pub use session::SessionAccess;
pub use tasks::TasksTools;
pub use tools::{Permission, ToolDef, ToolResult, ToolSet};
