//! Tests of our own agent. The model clients run against a local server that plays scripted streams (see
//! `server`), so nothing here calls a real API or needs a key.
mod anthropic;
mod context;
mod http;
mod openai;
mod permission;
mod server;
mod session;
mod sse;
mod store;
mod support;
mod tools;
