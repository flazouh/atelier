use std::time::{SystemTime, UNIX_EPOCH};

use atelier_agents::session::{ToolCall, ToolOutput, ToolStatus};
use atelier_capabilities::card::{failed_title, resolve, running_title};
use atelier_ui::{ToolCardData, ToolCardState, ToolNode, ToolText};
use serde_json::{Value, json};

use super::{
    cards::{card, fits, nouns, tool_of},
    map,
    providers::{Place, place_of, provider},
    types::Built,
};

/// The milliseconds since the epoch, for the relative times in a card.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The card for `call`, or `None` when the call keeps the plain row: it is not a gateway tool, the tool has no card, or what
/// it returned is not the shape its card draws. `waiting` is set while the call waits for the reader to allow it. `origin` says
/// whose agent made the call, as "Alex's agent". `now` is the time in milliseconds.
pub fn card_for(
    call: &ToolCall,
    output: Option<&ToolOutput>,
    waiting: bool,
    origin: Option<String>,
    now: i64,
) -> Option<Built> {
    let tool = tool_of(&call.name)?;
    let card = card(tool)?;
    let capability = tool.split('_').next().unwrap_or_default();
    let head = |state, title: String, place: Option<Place>, body: Option<ToolNode>| {
        let (provider, _) = provider(capability, place.as_ref());
        Built {
            data: ToolCardData {
                provider,
                title: title.into(),
                state,
                origin: origin.clone().map(Into::into),
                body,
                footer: Vec::new(),
            },
            presses: Vec::new(),
        }
    };
    let asked = place_of(None, &call.input);
    if waiting {
        return Some(head(
            ToolCardState::Waiting,
            running_title(card),
            asked,
            None,
        ));
    }
    let failed = call.status == ToolStatus::Failed || output.is_some_and(|o| o.is_error);
    match (call.status, output) {
        (ToolStatus::Pending | ToolStatus::Running, _) => Some(head(
            ToolCardState::Running,
            running_title(card),
            asked,
            None,
        )),
        (_, Some(output)) if failed => {
            // What went wrong is the text the agent read: a sentence that names the cause, shown quietly.
            let why = (!output.text.trim().is_empty()).then(|| ToolNode::Text {
                value: output.text.trim().to_string().into(),
                style: ToolText::Muted,
                max_lines: Some(3),
            });
            Some(head(ToolCardState::Failed, failed_title(card), asked, why))
        }
        (_, None) if failed => Some(head(ToolCardState::Failed, failed_title(card), asked, None)),
        (_, Some(output)) => {
            let mut result: Value = serde_json::from_str(&output.text).ok()?;
            if !fits(tool, &result) {
                return None;
            }
            let (provider, named) =
                provider(capability, place_of(Some(&result), &call.input).as_ref());
            facts(tool, &mut result, &provider, named);
            let rendered = resolve(card, &result, now);
            let mut presses = Vec::new();
            let body = rendered
                .body
                .as_ref()
                .and_then(|b| map::node(b, &mut presses));
            let footer = map::footer(&rendered.footer, &mut presses);
            Some(Built {
                data: ToolCardData {
                    provider,
                    title: rendered.title.trim().to_string().into(),
                    state: ToolCardState::Done,
                    origin: origin.map(Into::into),
                    body,
                    footer,
                },
                presses,
            })
        }
        (_, None) => None,
    }
}

/// What the app adds to a result before a card reads it, because a card cannot count or name: `_provider` (its `name` and
/// `account`), `_found` ("3 tasks") and `_in` ("in Linear", or nothing when the call did not say where).
fn facts(tool: &str, result: &mut Value, provider: &atelier_ui::ToolProvider, named: bool) {
    let found = result.get("items").and_then(Value::as_array).map(|items| {
        let more = if result.get("next_cursor").is_some_and(|c| !c.is_null()) {
            "+"
        } else {
            ""
        };
        let (one, many) = nouns(tool);
        let n = items.len();
        match (n, more) {
            (0, "") => format!("no {many}"),
            (1, "") => format!("1 {one}"),
            _ => format!("{n}{more} {many}"),
        }
    });
    if let Some(object) = result.as_object_mut() {
        object.insert("_provider".into(), json!({ "name": provider.name.to_string(), "account": provider.account.as_ref().map(|a| a.to_string()) }));
        object.insert("_found".into(), json!(found.unwrap_or_default()));
        object.insert(
            "_in".into(),
            json!(if named {
                format!("in {}", provider.name)
            } else {
                String::new()
            }),
        );
    }
}
