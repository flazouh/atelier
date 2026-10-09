use super::{
    Card, Resolved, ResolvedAction, Tone, failed_title, from_json, resolve, running_title, validate,
};
use serde_json::json;

const SEARCH_CARD: &str =
    include_str!("../../../../docs/capabilities/sentry.search_issues.card.json");
const GET_CARD: &str = include_str!("../../../../docs/capabilities/sentry.get_issue.card.json");
const SEARCH_RESULT: &str = include_str!("../../../../docs/capabilities/sentry.sample-result.json");
const GET_RESULT: &str =
    include_str!("../../../../docs/capabilities/sentry.get_issue.sample-result.json");

const THREE_HOURS: i64 = 3 * 3_600_000;
const NOW: i64 = 1_760_000_000_000 + THREE_HOURS;

fn search() -> (Card, serde_json::Value) {
    (
        from_json(SEARCH_CARD).expect("the search card"),
        serde_json::from_str(SEARCH_RESULT).unwrap(),
    )
}

fn texts(node: &Resolved, out: &mut Vec<String>) {
    match node {
        Resolved::Stack { children, .. } => children.iter().for_each(|c| texts(c, out)),
        Resolved::Text { value, .. } | Resolved::Badge { value, .. } => out.push(value.clone()),
        Resolved::Metric { label, value } => out.push(format!("{label}: {value}")),
        Resolved::List { rows, .. } => rows.iter().for_each(|r| texts(&r.node, out)),
        _ => {}
    }
}

#[test]
fn both_sentry_cards_are_valid() {
    assert!(from_json(SEARCH_CARD).is_ok());
    assert!(from_json(GET_CARD).is_ok());
}

#[test]
fn the_search_card_draws_one_row_per_issue_with_its_title_badges_and_open_action() {
    let (card, result) = search();
    let drawn = resolve(&card, &result, NOW);
    assert_eq!(drawn.title, "Searched Sentry, 2 issues");
    let Some(Resolved::List { rows, hidden, .. }) = &drawn.body else {
        panic!("a list: {:?}", drawn.body)
    };
    assert_eq!((rows.len(), *hidden), (2, 0));

    let mut first = Vec::new();
    texts(&rows[0].node, &mut first);
    assert_eq!(
        first,
        [
            "TypeError: Cannot read properties of undefined (reading 'id')",
            "web · last seen 3 hours ago",
            "142 events",
            "regressed"
        ]
    );
    let Resolved::Stack { children, .. } = &rows[0].node else {
        panic!("a stack")
    };
    assert_eq!(
        children[0],
        Resolved::Icon {
            name: "bug".into(),
            tone: Tone::Danger
        },
        "an error shows the red bug, not the warning icon"
    );
    assert_eq!(
        rows[0].on_click,
        Some(ResolvedAction::Open {
            label: "Open".into(),
            reference: "plugin:sentry:acme:WEB-1A".into()
        })
    );

    let mut second = Vec::new();
    texts(&rows[1].node, &mut second);
    assert_eq!(
        second,
        [
            "Slow query on /v1/orders",
            "api · last seen 5 hours ago",
            "31 events"
        ],
        "no regressed badge when it did not regress"
    );
    let Resolved::Stack { children, .. } = &rows[1].node else {
        panic!("a stack")
    };
    assert_eq!(
        children[0],
        Resolved::Icon {
            name: "warning".into(),
            tone: Tone::Warning
        }
    );
}

#[test]
fn the_get_card_draws_metrics_a_stack_excerpt_and_three_footer_actions() {
    let card = from_json(GET_CARD).unwrap();
    let result: serde_json::Value = serde_json::from_str(GET_RESULT).unwrap();
    let drawn = resolve(&card, &result, 1_759_900_000_000 + 2 * 86_400_000);
    assert_eq!(
        drawn.title,
        "WEB-1A: TypeError: Cannot read properties of undefined (reading 'id')"
    );
    let mut lines = Vec::new();
    texts(drawn.body.as_ref().unwrap(), &mut lines);
    assert!(lines.contains(&"Events: 1.5k".to_string()), "{lines:?}");
    assert!(
        lines.contains(&"Users: 87".to_string())
            && lines.contains(&"First seen: 2 days ago".to_string()),
        "{lines:?}"
    );
    assert!(lines.contains(&"error".to_string()) && lines.contains(&"unresolved".to_string()));
    assert_eq!(
        drawn.footer,
        vec![
            ResolvedAction::Open {
                label: "Open in Atelier".into(),
                reference: "plugin:sentry:acme:WEB-1A".into()
            },
            ResolvedAction::OpenUrl {
                label: "Open in Sentry".into(),
                url: "https://sentry.io/organizations/acme/issues/1/".into()
            },
            ResolvedAction::Call {
                label: "Resolve".into(),
                tool: "sentry.resolve_issue".into(),
                args: [("issue".to_string(), "plugin:sentry:acme:WEB-1A".to_string())].into()
            },
        ]
    );
}

#[test]
fn a_result_of_the_wrong_shape_draws_without_failing() {
    let (card, _) = search();
    let drawn = resolve(&card, &json!({ "unexpected": true }), NOW);
    assert_eq!(
        drawn.title, "Searched Sentry,  issues",
        "a hole that finds nothing is empty"
    );
    assert_eq!(
        drawn.body,
        Some(Resolved::List {
            rows: vec![],
            hidden: 0,
            empty: Some("No issues match.".into())
        })
    );
    let get = from_json(GET_CARD).unwrap();
    let drawn = resolve(&get, &json!(null), NOW);
    assert_eq!(
        drawn.footer.len(),
        1,
        "only the call action stays: the others have no ref or url to open"
    );
}

#[test]
fn a_link_that_is_not_https_is_dropped() {
    let card = from_json(GET_CARD).unwrap();
    let mut result: serde_json::Value = serde_json::from_str(GET_RESULT).unwrap();
    result["permalink"] = json!("javascript:alert(1)");
    let drawn = resolve(&card, &result, NOW);
    assert!(
        drawn
            .footer
            .iter()
            .all(|a| !matches!(a, ResolvedAction::OpenUrl { .. }))
    );
    result["permalink"] = json!("http://sentry.io/x");
    assert!(
        resolve(&card, &result, NOW)
            .footer
            .iter()
            .all(|a| !matches!(a, ResolvedAction::OpenUrl { .. })),
        "plain http is dropped too"
    );
}

#[test]
fn a_long_list_shows_fifty_rows_and_says_how_many_are_hidden() {
    let (card, _) = search();
    let issues: Vec<_> = (0..60).map(|i| json!({ "ref": format!("plugin:sentry:acme:I-{i}"), "title": format!("Issue {i}"), "level": "error", "project": "web", "last_seen": 0, "events": i })).collect();
    let drawn = resolve(&card, &json!({ "total": 60, "issues": issues }), NOW);
    let Some(Resolved::List { rows, hidden, .. }) = drawn.body else {
        panic!("a list")
    };
    assert_eq!((rows.len(), hidden), (10, 50), "this card asks for 10 rows");
}

#[test]
fn the_state_titles_have_defaults() {
    let (card, _) = search();
    assert_eq!(running_title(&card), "Searching Sentry…");
    assert_eq!(failed_title(&card), "Sentry search failed");
    let bare = from_json(r#"{ "version": 1, "tool": "x.y", "done": { "title": "ok" } }"#).unwrap();
    assert_eq!(
        (running_title(&bare).as_str(), failed_title(&bare).as_str()),
        ("Running x.y…", "x.y failed")
    );
}

#[test]
fn times_counts_sizes_and_durations_print_for_people() {
    let card = from_json(
        r#"{ "version": 1, "tool": "t", "done": { "title": { "template": "{$.a|count} {$.b|count} {$.c|bytes} {$.d|duration_ms} {$.e|percent} {$.f|absolute_time} {$.g|relative_time} {$.h|relative_time}" } } }"#,
    )
    .unwrap();
    let result = json!({ "a": 999, "b": 2_500_000, "c": 1_572_864, "d": 1200, "e": 41.6, "f": 1_760_000_000_000_i64, "g": 1_000_000, "h": 1_000_000 - 100 });
    let now = 1_000_000 + 30_000;
    assert_eq!(
        resolve(&card, &result, now).title,
        "999 2.5M 1.5 MB 1.2 s 42% 2025-10-09 08:53 UTC just now just now"
    );
    let result = json!({ "g": 0, "h": 0 });
    let t = resolve(&card, &result, 31 * 86_400_000).title;
    assert!(
        t.ends_with("1970-01-01 1970-01-01"),
        "a month old prints the date: {t}"
    );
}

#[test]
fn a_card_that_is_too_deep_or_too_wide_is_refused_with_every_reason() {
    let mut node = json!({ "type": "divider" });
    for _ in 0..7 {
        node = json!({ "type": "stack", "children": [node] });
    }
    let json =
        json!({ "version": 1, "tool": "t", "done": { "title": "t", "body": node } }).to_string();
    let err = from_json(&json).unwrap_err();
    assert!(err.contains("levels deep"), "{err}");

    let wide = json!({ "version": 1, "tool": "t", "done": { "title": "t", "body": { "type": "stack", "children": vec![json!({ "type": "divider" }); 13] } } }).to_string();
    assert!(from_json(&wide).unwrap_err().contains("13 children"));

    let card = from_json(r#"{ "version": 1, "tool": "t", "done": { "title": "t" } }"#).unwrap();
    assert!(validate(&card).is_ok());
    assert!(
        from_json(r#"{ "version": 2, "tool": "t", "done": { "title": "t" } }"#)
            .unwrap_err()
            .contains("version 2")
    );
}

#[test]
fn an_unknown_node_or_tone_does_not_parse() {
    assert!(from_json(r#"{ "version": 1, "tool": "t", "done": { "title": "t", "body": { "type": "script", "src": "x" } } }"#).is_err());
    assert!(from_json(r#"{ "version": 1, "tool": "t", "done": { "title": "t", "body": { "type": "badge", "value": "x", "tone": "pink" } } }"#).is_err());
}

#[test]
fn a_resolved_card_is_plain_json_for_the_other_renderers() {
    let (card, result) = search();
    let json = serde_json::to_value(resolve(&card, &result, NOW)).unwrap();
    assert_eq!(json["title"], "Searched Sentry, 2 issues");
    assert_eq!(json["body"]["type"], "list");
    assert_eq!(json["body"]["rows"][0]["on_click"]["type"], "open");
    assert_eq!(
        json["body"]["rows"][0]["node"]["children"][0]["tone"],
        "danger"
    );
}
