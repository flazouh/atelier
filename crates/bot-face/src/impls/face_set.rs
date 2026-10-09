use gpui_kit::Rgba;

use crate::enums::Mood;
use crate::structs::{BotModel, FaceData, FaceSet, StateDef};

impl FaceSet {
    /// Reads a `faces.v1.json` text.
    pub fn from_json(json: &str) -> Result<FaceSet, String> {
        let data: FaceData = serde_json::from_str(json).map_err(|e| format!("the face data does not parse: {e}"))?;
        if data.version != 1 {
            return Err(format!("face data version {} is not known", data.version));
        }
        let grey = Rgba::try_from(data.grey.as_str()).map_err(|e| format!("bad grey: {e}"))?;
        let mut states = [StateDef { speed: 1.0, amount: 1.0 }; 6];
        for mood in Mood::ALL {
            states[mood.index()] = *data.states.get(mood.name()).ok_or_else(|| format!("no state `{}`", mood.name()))?;
        }
        let bots = data.bots.iter().map(BotModel::from_def).collect::<Result<Vec<_>, _>>()?;
        Ok(FaceSet { bots, states, grey, mix: data.colour_mix_with_grey })
    }

    /// The bot with this id.
    pub fn bot(&self, id: &str) -> Option<&BotModel> {
        self.bots.iter().find(|b| b.id == id)
    }

    /// The speed and the amount of a mood.
    pub fn state(&self, mood: Mood) -> StateDef {
        self.states[mood.index()]
    }

    /// The colour a bot's body is drawn in: its own colour, mixed a little toward the grey.
    pub fn body_colour(&self, bot: &BotModel) -> Rgba {
        crate::structs::Palette::mixed(bot.colour, self.grey, self.mix)
    }
}
