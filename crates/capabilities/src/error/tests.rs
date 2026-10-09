use super::*;

#[test]
fn an_error_is_tagged_by_kind_in_json() {
    let json = serde_json::to_value(CapError::RateLimited {
        retry_after_ms: 1500,
    })
    .unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "kind": "rate_limited", "retry_after_ms": 1500 })
    );
    let back: CapError = serde_json::from_value(json).unwrap();
    assert_eq!(back.to_string(), "too many requests, try again in 2 s");
}
