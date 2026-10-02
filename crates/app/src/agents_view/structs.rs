use std::collections::BTreeMap;

/// What the reader chose for projects' badges, by place: a colour, an image file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Badges {
    pub colors: BTreeMap<String, u8>,
    pub icons: BTreeMap<String, String>,
}

impl Badges {
    pub fn saved(settings: &atelier_settings::Settings) -> Self {
        Self { colors: settings.project_colors.clone(), icons: settings.project_icons.clone() }
    }
}
