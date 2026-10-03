use std::sync::atomic::{AtomicU8, Ordering};

/// Defines [`Locale`] and [`Message`] from one list, so they cannot drift apart: each row is a variant, the field
/// of [`Message`] it names, its tag, its name in its own language, and whether it is written right to left.
macro_rules! locales {
    ($($variant:ident $field:ident $tag:literal $name:literal $rtl:literal),+ $(,)?) => {
        /// A language the app speaks.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Locale { $($variant),+ }

        impl Locale {
            /// Every locale, in tag order.
            pub const ALL: &'static [Locale] = &[$(Locale::$variant),+];

            /// The BCP 47 tag.
            pub fn tag(self) -> &'static str {
                match self { $(Locale::$variant => $tag),+ }
            }

            /// The language in its own words, for a picker.
            pub fn name(self) -> &'static str {
                match self { $(Locale::$variant => $name),+ }
            }

            /// Whether the language is written right to left.
            pub fn is_rtl(self) -> bool {
                match self { $(Locale::$variant => $rtl),+ }
            }
        }

        /// One string for every locale. A field left out is a compile error, and that is the point.
        #[derive(Clone, Copy, Debug)]
        pub struct Message { $(pub $field: &'static str),+ }

        impl Message {
            /// The string in `locale`.
            pub fn get(&self, locale: Locale) -> &'static str {
                match locale { $(Locale::$variant => self.$field),+ }
            }
        }

        /// The names of [`Message`]'s fields, in tag order: what a generated file writes.
        pub(crate) const FIELDS: &[&str] = &[$(stringify!($field)),+];
    };
}

locales! {
    Ar ar "ar" "العربية" true,
    Ca ca "ca" "Català" false,
    Cs cs "cs" "Čeština" false,
    Da da "da" "Dansk" false,
    De de "de" "Deutsch" false,
    El el "el" "Ελληνικά" false,
    En en "en" "English" false,
    Es es "es" "Español" false,
    Es419 es_419 "es-419" "Español (Latinoamérica)" false,
    Fi fi "fi" "Suomi" false,
    Fr fr "fr" "Français" false,
    FrCa fr_ca "fr-CA" "Français (Canada)" false,
    He he "he" "עברית" true,
    Hi hi "hi" "हिन्दी" false,
    Hr hr "hr" "Hrvatski" false,
    Hu hu "hu" "Magyar" false,
    Id id "id" "Bahasa Indonesia" false,
    It it "it" "Italiano" false,
    Ja ja "ja" "日本語" false,
    Ko ko "ko" "한국어" false,
    Ms ms "ms" "Bahasa Melayu" false,
    Nl nl "nl" "Nederlands" false,
    No no "no" "Norsk" false,
    Pl pl "pl" "Polski" false,
    PtBr pt_br "pt-BR" "Português (Brasil)" false,
    PtPt pt_pt "pt-PT" "Português (Portugal)" false,
    Ro ro "ro" "Română" false,
    Ru ru "ru" "Русский" false,
    Sk sk "sk" "Slovenčina" false,
    Sv sv "sv" "Svenska" false,
    Th th "th" "ไทย" false,
    Tr tr "tr" "Türkçe" false,
    Uk uk "uk" "Українська" false,
    Vi vi "vi" "Tiếng Việt" false,
    ZhCn zh_cn "zh-CN" "简体中文" false,
    ZhTw zh_tw "zh-TW" "繁體中文" false,
}

impl Locale {
    /// The locale a tag names, as a system or a browser writes it: `fr_CH.UTF-8`, `zh-Hant-HK`, `nb`, `en-GB`.
    /// The exact tag first, then the closest the app has, then the language alone; `None` where the app has no such language.
    pub fn from_tag(tag: &str) -> Option<Locale> {
        let tag = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-");
        let lower = tag.to_ascii_lowercase();
        if let Some(exact) = Self::ALL.iter().find(|l| l.tag().eq_ignore_ascii_case(&tag)) {
            return Some(*exact);
        }
        let parts: Vec<&str> = lower.split('-').collect();
        let language = *parts.first()?;
        let has = |part: &str| parts[1..].contains(&part);
        let region = parts[1..].iter().find(|p| p.len() == 2 || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))).copied();
        Some(match language {
            "zh" if has("hant") || matches!(region, Some("tw" | "hk" | "mo")) => Locale::ZhTw,
            "zh" => Locale::ZhCn,
            "pt" if region == Some("pt") => Locale::PtPt,
            "pt" => Locale::PtBr,
            "es" if matches!(region, Some("es")) || region.is_none() => Locale::Es,
            "es" => Locale::Es419,
            "fr" if region == Some("ca") => Locale::FrCa,
            "nb" | "nn" => Locale::No,
            "iw" => Locale::He,
            other => *Self::ALL.iter().find(|l| l.tag().eq_ignore_ascii_case(other))?,
        })
    }

    /// The first of `tags` the app speaks, else English: the system's languages in the reader's order.
    pub fn resolve<'a>(tags: impl IntoIterator<Item = &'a str>) -> Locale {
        tags.into_iter().find_map(Locale::from_tag).unwrap_or(Locale::En)
    }

    /// One more than its place in [`Locale::ALL`]: 0 is left for "never set".
    fn slot(self) -> u8 {
        Self::ALL.iter().position(|l| *l == self).map_or(0, |at| at as u8 + 1)
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The language the app shows now. English until [`set_current`] says otherwise.
pub fn current() -> Locale {
    match CURRENT.load(Ordering::Relaxed) {
        0 => Locale::En,
        slot => Locale::ALL.get(slot as usize - 1).copied().unwrap_or(Locale::En),
    }
}

/// Changes the language. Text that was read earlier stays as it was read.
pub fn set_current(locale: Locale) {
    CURRENT.store(locale.slot(), Ordering::Relaxed);
}
