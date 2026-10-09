use atelier_capabilities::CapError;

use super::from_reply_at;
use crate::runner::Reply;

fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> Reply {
    Reply {
        status,
        headers: headers
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect(),
        body: body.into(),
    }
}

#[test]
fn a_401_is_signed_out_and_a_404_is_not_found() {
    assert_eq!(
        from_reply_at(&reply(401, &[], r#"{"message":"Bad credentials"}"#), "x", 0),
        CapError::NotSignedIn
    );
    assert_eq!(
        from_reply_at(
            &reply(404, &[], r#"{"message":"Not Found"}"#),
            "issue #5",
            0
        ),
        CapError::not_found("issue #5")
    );
}

#[test]
fn a_403_that_names_a_rate_limit_waits_as_long_as_github_says() {
    let by_header = reply(403, &[("retry-after", "7")], r#"{"message":"abuse"}"#);
    assert_eq!(
        from_reply_at(&by_header, "x", 0),
        CapError::RateLimited {
            retry_after_ms: 7000
        }
    );
    let by_reset = reply(
        403,
        &[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "1030"),
        ],
        "{}",
    );
    assert_eq!(
        from_reply_at(&by_reset, "x", 1000),
        CapError::RateLimited {
            retry_after_ms: 30_000
        }
    );
    let by_words = reply(
        403,
        &[],
        r#"{"message":"API rate limit exceeded for user"}"#,
    );
    assert_eq!(
        from_reply_at(&by_words, "x", 0),
        CapError::RateLimited {
            retry_after_ms: 60_000
        }
    );
    assert!(matches!(
        from_reply_at(&reply(429, &[], ""), "x", 0),
        CapError::RateLimited { .. }
    ));
}

#[test]
fn a_403_with_no_limit_is_the_providers_own_refusal() {
    let denied = reply(
        403,
        &[],
        r#"{"message":"Resource not accessible by integration"}"#,
    );
    assert_eq!(
        from_reply_at(&denied, "x", 0),
        CapError::Provider {
            code: "403".into(),
            message: "Resource not accessible by integration".into()
        }
    );
}

#[test]
fn a_422_names_the_field_and_other_statuses_keep_the_message() {
    let invalid = reply(
        422,
        &[],
        r#"{"message":"Validation Failed","errors":[{"field":"title","code":"missing_field"}]}"#,
    );
    assert_eq!(from_reply_at(&invalid, "x", 0), CapError::invalid("title"));
    assert_eq!(
        from_reply_at(&reply(502, &[], "<html>"), "x", 0),
        CapError::Provider {
            code: "502".into(),
            message: "HTTP 502".into()
        }
    );
}
