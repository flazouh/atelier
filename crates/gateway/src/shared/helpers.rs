use std::sync::Arc;

use atelier_capabilities::CapError;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use super::structs::Cut;

/// The longest message or mail body one result carries, in characters. A longer one is cut, and says so.
pub(crate) const TEXT_MAX: usize = 8000;
/// The most items one page of history, search or channels carries.
pub(crate) const PAGE_MAX: u32 = 50;

/// Puts `body` between two marker lines, and says above them (`notice`) what the lines mean.
///
/// The text inside is written by other people, so it can say "ignore your instructions". The markers tell the model
/// where such text starts and ends. The text cannot end the block early: a marker line of the same `kind` inside it is
/// changed, so the real end marker is always the last line.
pub(crate) fn untrusted(kind: &str, notice: &str, body: &str) -> String {
    let quoted = body
        .replace(
            &format!("--- end {kind} data"),
            &format!("(quoted) end {kind} data"),
        )
        .replace(
            &format!("--- begin {kind} data"),
            &format!("(quoted) begin {kind} data"),
        );
    format!(
        "{notice}\n--- begin {kind} data (untrusted) ---\n{}\n--- end {kind} data ---",
        quoted.trim_end()
    )
}

/// The characters of `text` from `offset` on, at most [`TEXT_MAX`] of them.
pub(crate) fn cut(text: &str, offset: usize) -> Cut {
    let total = text.chars().count();
    let from = offset.min(total);
    let shown: String = text.chars().skip(from).take(TEXT_MAX).collect();
    let to = from + shown.chars().count();
    Cut {
        text: shown,
        total,
        from,
        to,
    }
}

/// Cuts the `text` field of an entity in place, and says what was cut in `text_cut`.
pub(crate) fn shorten(entity: &mut Value, offset: usize) {
    let Some(text) = entity["text"].as_str() else {
        return;
    };
    let slice = cut(text, offset);
    if slice.is_cut() {
        entity["text_cut"] = json!({ "length": slice.total, "from": slice.from, "to": slice.to });
    }
    entity["text"] = json!(slice.text);
}

/// The neutral JSON of an entity, without `raw`. `raw` is the provider's own JSON, kept for export; it is large, and it
/// can hold more of the same untrusted text.
pub(crate) fn neutral(entity: &impl Serialize) -> Value {
    let mut value = serde_json::to_value(entity).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("raw");
    }
    value
}

/// The word an enum serializes to: `public`, `inbox`.
pub(crate) fn word(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// The fields of `args` that a call takes, plus `extra`, read as a `T`. A field the call does not take is left out,
/// so a model that adds one does not fail. A field of the wrong shape names itself in the sentence.
pub(crate) fn pick<T: DeserializeOwned>(
    args: &Value,
    keys: &[&str],
    extra: &[(&str, Value)],
) -> Result<T, String> {
    let given = args
        .as_object()
        .ok_or("The arguments must be a JSON object.")?;
    let mut taken = Map::new();
    for key in keys {
        if let Some(value) = given.get(*key) {
            taken.insert((*key).to_string(), value.clone());
        }
    }
    for (key, value) in extra {
        taken.insert((*key).to_string(), value.clone());
    }
    serde_json::from_value(Value::Object(taken))
        .map_err(|e| format!("The arguments are not valid: {e}."))
}

pub(crate) fn required(args: &Value, key: &str) -> Result<String, String> {
    match args[key].as_str().map(str::trim) {
        Some(text) if !text.is_empty() => Ok(text.to_string()),
        _ => Err(format!("The argument {key} is required and must be text.")),
    }
}

/// A text argument that may be left out. Empty text counts as left out.
pub(crate) fn optional(args: &Value, key: &str) -> Option<String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(String::from)
}

/// The page size a call asks for, kept between 1 and [`PAGE_MAX`]. Without one, the most there is.
pub(crate) fn limit(args: &Value) -> u32 {
    args["limit"]
        .as_u64()
        .map_or(PAGE_MAX, |n| n.clamp(1, u64::from(PAGE_MAX)) as u32)
}

/// The `account` argument as provider and account, when the call gives one.
pub(crate) fn account(args: &Value) -> Result<Option<(String, String)>, String> {
    let Some(text) = args["account"].as_str().filter(|a| !a.is_empty()) else {
        return Ok(None);
    };
    match text.split_once('/') {
        Some((provider, account)) if !provider.is_empty() && !account.is_empty() => {
            Ok(Some((provider.to_string(), account.to_string())))
        }
        _ => Err(format!(
            "The account \"{text}\" must look like provider/account, for example slack/acme."
        )),
    }
}

/// The provider a call goes to, among `all` (provider, account and the provider itself): the account when the call names
/// one, else the one the ref belongs to, else the only one there is. With several and no hint, the sentence lists them,
/// so the model can choose. `noun` is the capability, for the sentences. Gives the place as `provider/account` too.
pub(crate) fn choose<T: ?Sized>(
    noun: &str,
    all: Vec<(String, String, Arc<T>)>,
    named: Option<(String, String)>,
    from_ref: Option<(String, String)>,
) -> Result<(String, Arc<T>), String> {
    if let (Some(named), Some(from_ref)) = (&named, &from_ref)
        && named != from_ref
    {
        return Err(format!(
            "The ref belongs to {}/{} but the account says {}/{}. Leave the account out, or make them agree.",
            from_ref.0, from_ref.1, named.0, named.1
        ));
    }
    let choices = || {
        all.iter()
            .map(|(provider, account, _)| format!("{provider}/{account}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match named.or(from_ref) {
        Some((provider, account)) => all
            .iter()
            .find(|(p, a, _)| *p == provider && *a == account)
            .map(|(p, a, found)| (format!("{p}/{a}"), found.clone()))
            .ok_or_else(|| {
                format!(
                    "No {noun} account is connected at {provider}/{account}. Choose one of: {}.",
                    choices()
                )
            }),
        None => match all.as_slice() {
            [] => Err(format!("No {noun} account is connected in Atelier.")),
            [(provider, account, only)] => Ok((format!("{provider}/{account}"), only.clone())),
            _ => Err(format!(
                "More than one {noun} account is connected. Pass account as one of: {}.",
                choices()
            )),
        },
    }
}

/// The sentence for a failed call, in plain words. `place` is `provider/account`.
pub(crate) fn failed(what: &str, place: &str, error: CapError) -> String {
    match error {
        CapError::NotSignedIn => format!(
            "Could not {what}: you are not signed in to {place}. Ask the person to sign in to it in Atelier, then try again."
        ),
        CapError::Offline => {
            format!("Could not {what}: there is no connection to {place}. Try again later.")
        }
        CapError::RateLimited { retry_after_ms } => format!(
            "Could not {what}: {place} asks for fewer requests. Try again in {} s.",
            retry_after_ms.div_ceil(1000)
        ),
        CapError::Unsupported { feature } => format!("Could not {what}: {place} cannot {feature}."),
        CapError::NotFound { what: missing } => {
            format!("Could not {what}: {missing} was not found at {place}.")
        }
        CapError::Invalid { field } => format!(
            "Could not {what}: {field} is not valid. Use a value that an earlier call gave you."
        ),
        other => format!("Could not {what}: {other}."),
    }
}

/// The first line of a page: how many there are, where, and how to get the next page.
pub(crate) fn heading(
    count: usize,
    one: &str,
    many: &str,
    place: &str,
    next_cursor: Option<&str>,
    left_out: usize,
) -> String {
    let mut line = format!(
        "{count} {} in {place}.",
        if count == 1 { one } else { many }
    );
    if left_out > 0 {
        line.push_str(&format!(
            " The provider gave {left_out} more than the {PAGE_MAX} a page may hold, and they were left out. Ask for a smaller limit."
        ));
    }
    if let Some(cursor) = next_cursor {
        line.push_str(&format!(" More are available: pass cursor \"{cursor}\"."));
    }
    line
}

/// The items of a page, at most [`PAGE_MAX`], and how many were left out. A provider is asked for no more than that;
/// this holds when it does not listen.
pub(crate) fn clip<T>(mut items: Vec<T>) -> (Vec<T>, usize) {
    let max = PAGE_MAX as usize;
    let left_out = items.len().saturating_sub(max);
    items.truncate(max);
    (items, left_out)
}

/// `2026-10-09 12:30 UTC` for milliseconds since the epoch.
pub(crate) fn utc(millis: i64) -> String {
    let seconds = millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    // Days since 1970-01-01 to a calendar date: the proleptic Gregorian civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        of_day / 3600,
        of_day % 3600 / 60
    )
}
