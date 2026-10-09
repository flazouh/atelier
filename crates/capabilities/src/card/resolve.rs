use std::collections::BTreeMap;

use super::{
    format::{eval, fill, same, show, text_of},
    structs::{
        Action, Card, Condition, Literal, Node, Rendered, Resolved, ResolvedAction, Row, Value,
    },
};

/// The most rows a list shows. The rest show as "Show more".
pub(super) const LIST_MAX: usize = 50;

/// The title while the tool runs.
pub fn running_title(card: &Card) -> String {
    match &card.running {
        Some(line) => text(&line.title, &serde_json::Value::Null, 0),
        None => format!("Running {}…", card.tool),
    }
}

/// The title when the tool fails.
pub fn failed_title(card: &Card) -> String {
    match &card.failed {
        Some(line) => text(&line.title, &serde_json::Value::Null, 0),
        None => format!("{} failed", card.tool),
    }
}

/// A card in its `done` state, drawn from the tool's `result`. `now` is milliseconds since the epoch, for relative times.
/// A path that finds nothing shows its default or nothing, so a result of the wrong shape never fails.
pub fn resolve(card: &Card, result: &serde_json::Value, now: i64) -> Rendered {
    Rendered {
        title: text(&card.done.title, result, now),
        body: card
            .done
            .body
            .as_ref()
            .and_then(|node| node_of(node, result, now)),
        footer: card
            .done
            .footer
            .iter()
            .filter_map(|a| action_of(a, result, now))
            .collect(),
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Text(s) => s.clone(),
        Literal::Number(n) => text_of(&serde_json::Value::Number(n.clone())),
        Literal::Flag(b) => b.to_string(),
    }
}

fn text(value: &Value, root: &serde_json::Value, now: i64) -> String {
    match value {
        Value::Literal(literal) => literal_text(literal),
        Value::Template(t) => fill(&t.template, root, now),
        Value::Path(p) => {
            let found = eval(root, &p.path)
                .map(|v| show(v, p.format, now))
                .unwrap_or_default();
            match (found.is_empty(), &p.default) {
                (true, Some(default)) => literal_text(default),
                _ => found,
            }
        }
    }
}

fn holds(when: &Option<Condition>, root: &serde_json::Value) -> bool {
    let Some(c) = when else { return true };
    let found = eval(root, &c.path).filter(|v| !v.is_null());
    let exists_ok = c.exists.is_none_or(|want| found.is_some() == want);
    let equals_ok = c
        .equals
        .as_ref()
        .is_none_or(|want| found.is_some_and(|v| same(v, want)));
    exists_ok && equals_ok
}

fn https(url: String) -> Option<String> {
    url.get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
        .then_some(url)
}

fn action_of(action: &Action, root: &serde_json::Value, now: i64) -> Option<ResolvedAction> {
    match action {
        Action::Open { label, reference } => {
            let reference = text(reference, root, now);
            (!reference.is_empty()).then(|| ResolvedAction::Open {
                label: label.clone(),
                reference,
            })
        }
        Action::OpenUrl { label, url } => {
            https(text(url, root, now)).map(|url| ResolvedAction::OpenUrl {
                label: label.clone(),
                url,
            })
        }
        Action::Call { label, tool, args } => {
            let args: BTreeMap<String, String> = args
                .iter()
                .map(|(k, v)| (k.clone(), text(v, root, now)))
                .collect();
            Some(ResolvedAction::Call {
                label: label.clone(),
                tool: tool.clone(),
                args,
            })
        }
    }
}

fn node_of(node: &Node, root: &serde_json::Value, now: i64) -> Option<Resolved> {
    match node {
        Node::Stack {
            direction,
            gap,
            children,
            when,
        } => holds(when, root).then(|| Resolved::Stack {
            direction: *direction,
            gap: *gap,
            children: children
                .iter()
                .filter_map(|c| node_of(c, root, now))
                .collect(),
        }),
        Node::Text {
            value,
            style,
            max_lines,
            when,
        } => {
            let value = text(value, root, now);
            (holds(when, root) && !value.is_empty()).then_some(Resolved::Text {
                value,
                style: *style,
                max_lines: *max_lines,
            })
        }
        Node::Badge { value, tone, when } => {
            let value = text(value, root, now);
            (holds(when, root) && !value.is_empty())
                .then_some(Resolved::Badge { value, tone: *tone })
        }
        Node::Metric { label, value, when } => {
            let value = text(value, root, now);
            (holds(when, root) && !value.is_empty()).then(|| Resolved::Metric {
                label: label.clone(),
                value,
            })
        }
        Node::Icon { name, tone, when } => holds(when, root).then(|| Resolved::Icon {
            name: name.clone(),
            tone: *tone,
        }),
        Node::Avatar { name, url, when } => {
            let name = text(name, root, now);
            (holds(when, root) && !name.is_empty()).then(|| Resolved::Avatar {
                name,
                url: url.as_ref().and_then(|u| https(text(u, root, now))),
            })
        }
        Node::Code {
            value,
            language,
            max_lines,
            when,
        } => {
            let value = text(value, root, now);
            (holds(when, root) && !value.is_empty()).then(|| Resolved::Code {
                value,
                language: language.clone(),
                max_lines: *max_lines,
            })
        }
        Node::List {
            items,
            item,
            limit,
            empty,
            on_item,
            when,
        } => {
            if !holds(when, root) {
                return None;
            }
            let all = eval(root, items)
                .and_then(|v| v.as_array())
                .map(Vec::as_slice)
                .unwrap_or_default();
            let shown = all
                .len()
                .min(usize::from(limit.unwrap_or(LIST_MAX as u8)).min(LIST_MAX));
            let rows = all[..shown]
                .iter()
                .filter_map(|row| {
                    Some(Row {
                        node: node_of(item, row, now)?,
                        on_click: on_item.as_ref().and_then(|a| action_of(a, row, now)),
                    })
                })
                .collect();
            Some(Resolved::List {
                rows,
                hidden: all.len() - shown,
                empty: empty.clone(),
            })
        }
        Node::Button { action, when } => {
            if !holds(when, root) {
                return None;
            }
            action_of(action, root, now).map(|action| Resolved::Button { action })
        }
        Node::Divider { when } => holds(when, root).then_some(Resolved::Divider),
    }
}
