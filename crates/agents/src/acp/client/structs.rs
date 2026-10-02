use serde_json::Value;

pub(in super::super) struct Outgoing {
    pub method: &'static str,
    pub params: Value,
}
