use std::fmt;
use std::str::FromStr;

use crate::consts::ID_MAX;
use crate::structs::BotId;

impl BotId {
    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for BotId {
    type Err = String;

    /// 1 to 32 characters: `a` to `z`, `0` to `9` and `-`. It does not start or end with `-`.
    fn from_str(text: &str) -> Result<Self, String> {
        let ok_chars = text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if text.is_empty()
            || text.len() > ID_MAX
            || !ok_chars
            || text.starts_with('-')
            || text.ends_with('-')
        {
            return Err(format!(
                "`{text}` is not a good id: use 1 to {ID_MAX} letters a to z, digits or hyphens, with no hyphen at either end"
            ));
        }
        Ok(BotId(text.to_string()))
    }
}

impl fmt::Display for BotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for BotId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<BotId> for String {
    fn from(id: BotId) -> String {
        id.0
    }
}

impl serde::Serialize for BotId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for BotId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
