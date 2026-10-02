use super::structs::{Reply, Request};
use super::types::TransportError;

pub trait Transport: Send + Sync {
    fn send(&self, request: &Request) -> Result<Reply, TransportError>;
}
