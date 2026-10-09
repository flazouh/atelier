use super::*;

#[test]
fn a_reference_parses_back_to_its_parts_and_prints_the_same_text() {
    let r: Ref = "tasks:linear:acme:ENG-123".parse().unwrap();
    assert_eq!(
        (
            r.capability.as_str(),
            r.provider.as_str(),
            r.account.as_str(),
            r.id.as_str()
        ),
        ("tasks", "linear", "acme", "ENG-123")
    );
    assert_eq!(r.to_string(), "tasks:linear:acme:ENG-123");
}

#[test]
fn the_id_may_hold_colons() {
    let r: Ref = "messaging:slack:acme:C01:1760000000.0001".parse().unwrap();
    assert_eq!(r.id, "C01:1760000000.0001");
    assert_eq!(r.to_string(), "messaging:slack:acme:C01:1760000000.0001");
}

#[test]
fn a_short_or_empty_reference_is_refused() {
    assert!(matches!(
        "tasks:linear:acme".parse::<Ref>(),
        Err(RefError::Shape(_))
    ));
    assert!(matches!(
        "tasks:linear::ENG-1".parse::<Ref>(),
        Err(RefError::Part {
            part: "account",
            ..
        })
    ));
    assert!(
        matches!(
            "tasks:Linear:acme:1".parse::<Ref>(),
            Err(RefError::Part {
                part: "provider",
                ..
            })
        ),
        "provider names are lower case"
    );
    assert!(matches!(
        "tasks:linear:acme:".parse::<Ref>(),
        Err(RefError::Part { part: "id", .. })
    ));
}

#[test]
fn json_carries_a_reference_as_one_string() {
    let r: Ref = "plugin:sentry:acme:WEB-1A".parse().unwrap();
    assert_eq!(
        serde_json::to_string(&r).unwrap(),
        r#""plugin:sentry:acme:WEB-1A""#
    );
    assert_eq!(
        serde_json::from_str::<Ref>(r#""plugin:sentry:acme:WEB-1A""#).unwrap(),
        r
    );
    assert!(serde_json::from_str::<Ref>(r#""nope""#).is_err());
}
