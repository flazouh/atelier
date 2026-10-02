//! Chat Completions, streaming, for OpenAI and every service that copies it: OpenRouter, vLLM, Ollama,
//! Together, Groq. Tool calls arrive as fragments by index. Reasoning arrives as `reasoning_content`
//! (DeepSeek and others) or `reasoning` (OpenRouter), and is shown but not sent back.

mod helpers;
mod structs;
mod types;

pub use helpers::request_body;
pub use structs::{ChatState, OpenAiCompatible};
pub use types::{OPENAI_BASE, OPENROUTER_BASE};
