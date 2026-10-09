use atelier_ui::ToolProvider;

/// A provider the app knows: the id in a reference (`tasks:linear:acme:ENG-1` has `linear`), its name, the letter on its tile and
/// the colour of the tile as an index into the project palette. The tiles are a letter on a colour, not a copy of anyone's logo.
const KNOWN: [(&str, &str, &str, usize); 6] = [
    ("local", "Atelier", "A", 1),
    ("linear", "Linear", "L", 9),
    ("github", "GitHub", "G", 10),
    ("slack", "Slack", "S", 11),
    ("discord", "Discord", "D", 8),
    ("gmail", "Gmail", "G", 0),
];

/// What a call says about where it went: the provider and the account, as a reference or an `account` argument gives them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub provider: String,
    pub account: String,
}

/// The provider as the card shows it, and whether a call said which it is. A call that says nothing (an empty list, a call still
/// running) gets the capability's own name on a neutral tile; a provider that is not in the table gets its id, capitalised,
/// on a neutral tile.
pub fn provider(capability: &str, place: Option<&Place>) -> (ToolProvider, bool) {
    match place {
        Some(place) => {
            let account = Some(place.account.clone().into())
                .filter(|a: &gpui_kit::SharedString| !a.is_empty());
            match KNOWN.iter().find(|(id, ..)| *id == place.provider) {
                Some((_, name, letter, color)) => (
                    ToolProvider {
                        name: (*name).into(),
                        account,
                        letter: (*letter).into(),
                        color: Some(*color),
                    },
                    true,
                ),
                None => {
                    let name = capitalised(&place.provider);
                    let letter = name
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "·".into());
                    (
                        ToolProvider {
                            name: name.into(),
                            account,
                            letter: letter.into(),
                            color: None,
                        },
                        true,
                    )
                }
            }
        }
        None => {
            let name = capitalised(match capability {
                "tasks" => "tasks",
                "messaging" => "messages",
                "mail" => "mail",
                other => other,
            });
            let letter = name
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "·".into());
            (
                ToolProvider {
                    name: name.into(),
                    account: None,
                    letter: letter.into(),
                    color: None,
                },
                false,
            )
        }
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The place a result names: the first reference in it, which is `tasks:linear:acme:ENG-1`, and else the call's own arguments.
pub fn place_of(result: Option<&serde_json::Value>, input: &serde_json::Value) -> Option<Place> {
    result.and_then(first_ref).or_else(|| from_input(input))
}

fn place_in(text: &str) -> Option<Place> {
    let reference: atelier_capabilities::Ref = text.parse().ok()?;
    Some(Place {
        provider: reference.provider,
        account: reference.account,
    })
}

fn first_ref(value: &serde_json::Value) -> Option<Place> {
    match value {
        serde_json::Value::Object(map) => map
            .get("ref")
            .and_then(|r| r.as_str())
            .and_then(place_in)
            .or_else(|| map.values().find_map(first_ref)),
        serde_json::Value::Array(rows) => rows.iter().find_map(first_ref),
        _ => None,
    }
}

fn from_input(input: &serde_json::Value) -> Option<Place> {
    let object = input.as_object()?;
    // A reference in an argument says the place; so does `account`, as `provider/account`.
    ["ref", "channel", "message", "mailbox", "in_thread_of"]
        .iter()
        .find_map(|key| object.get(*key).and_then(|v| v.as_str()).and_then(place_in))
        .or_else(|| {
            let (provider, account) = object.get("account")?.as_str()?.split_once('/')?;
            Some(Place {
                provider: provider.to_string(),
                account: account.to_string(),
            })
        })
}
