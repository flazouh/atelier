use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};

use atelier_bot_face::{BotRuntime, FaceSet, Mood};
use atelier_bots::BotId;
use atelier_ui::{ActiveTheme, Badge, Segment, Segmented, scale::px, typography::TextSize};
use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, Rgba, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::super::{
    consts::{DETAIL_PAD, FACE_DATA, NO_FOLDER, NO_NOTES, NOTHING, PROFILE_FACE, READING, ROW_FACE},
    helpers::{access_words, face, face_of, harness_words, mood_words, provider_words, read_library, voice_words},
};
use super::{Entry, Library};

/// The bot library: the bots of the folder and the one chosen, with the mood its face shows. The folder is read off
/// the UI thread each time the view comes in front, so an edit on disk shows at the next opening.
pub struct BotsPage {
    faces: Rc<FaceSet>,
    root: Option<PathBuf>,
    library: Library,
    /// A read is on its way and nothing was read yet.
    loading: bool,
    selected: Option<BotId>,
    mood: Mood,
    /// The profile face's motion. A new choice gets a new one, so the bot grows in.
    runtime: Rc<RefCell<BotRuntime>>,
    started: Instant,
}

impl BotsPage {
    /// Opens the library on `root` and reads it. With no folder the page says so and reads nothing.
    pub fn new(root: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let faces = Rc::new(FaceSet::from_json(FACE_DATA).expect("the face data built into the app loads"));
        let page = Self {
            faces,
            loading: root.is_some(),
            root,
            library: Library::default(),
            selected: None,
            mood: Mood::Idle,
            runtime: Rc::new(RefCell::new(BotRuntime::new(1, 0.))),
            started: Instant::now(),
        };
        page.read(cx);
        page
    }

    /// The view came in front again: reads the folder once more, and keeps what it shows until it is read.
    pub fn refresh(&self, cx: &mut Context<Self>) {
        self.read(cx);
    }

    fn read(&self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        cx.spawn(async move |this, cx| {
            let library = cx.background_executor().spawn(async move { read_library(&root) }).await;
            drop(this.update(cx, |page, cx| {
                page.library = library;
                page.loading = false;
                let kept = page.selected.as_ref().is_some_and(|id| page.library.entries.iter().any(|e| e.bot.id == *id));
                if !kept && let Some(first) = page.library.entries.first().map(|e| e.bot.id.clone()) {
                    page.select(first, cx);
                }
                cx.notify();
            }));
        })
        .detach();
    }

    /// Every bot, in the order of their ids.
    pub fn entries(&self) -> &[Entry] {
        &self.library.entries
    }

    pub fn selected(&self) -> Option<&BotId> {
        self.selected.as_ref()
    }

    /// Why the folder could not be read, when it could not.
    pub fn error(&self) -> Option<&str> {
        self.library.error.as_deref()
    }

    /// Chooses a bot. Its face grows in, as a bot new on screen does.
    pub fn select(&mut self, id: BotId, cx: &mut Context<Self>) {
        if self.selected.as_ref() == Some(&id) {
            return;
        }
        let seed = self.library.entries.iter().position(|e| e.bot.id == id).unwrap_or(0) as u32 + 1;
        self.runtime = Rc::new(RefCell::new(BotRuntime::new(seed, 0.)));
        self.started = Instant::now();
        self.selected = Some(id);
        cx.notify();
    }

    fn set_mood(&mut self, mood: Mood, cx: &mut Context<Self>) {
        self.mood = mood;
        cx.notify();
    }

    fn chosen(&self) -> Option<&Entry> {
        let id = self.selected.as_ref()?;
        self.library.entries.iter().find(|e| e.bot.id == *id)
    }

    /// The face of a row: still, at the idle pose, drawn with the theme's ink.
    pub fn row_face(&self, entry: &Entry, cx: &Context<Self>) -> Option<AnyElement> {
        let model = face_of(&self.faces, &entry.bot)?;
        let ink = Rgba::from(cx.theme().foreground);
        Some(face(self.faces.clone(), model, Mood::Idle, None, ROW_FACE, ink).into_any_element())
    }

    /// The profile of the chosen bot, scrolling inside the card the shell puts it on.
    pub fn main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let body = match self.chosen() {
            Some(entry) => self.profile(entry, cx),
            None => {
                let (words, selector, colour) = match (&self.root, &self.library.error, self.loading) {
                    (_, Some(error), _) => (error.clone(), "bots-error", theme.danger),
                    (None, _, _) => (NO_FOLDER.into(), "bots-empty", theme.muted_foreground),
                    (_, None, true) => (READING.into(), "bots-empty", theme.muted_foreground),
                    (_, None, false) => (NOTHING.into(), "bots-empty", theme.muted_foreground),
                };
                div()
                    .debug_selector(move || selector.into())
                    .text_size(TextSize::Sm.font_size())
                    .text_color(colour)
                    .child(words)
                    .into_any_element()
            }
        };
        div()
            .id("bots-scroll")
            .debug_selector(|| "bots-view".into())
            .size_full()
            .overflow_y_scroll()
            .p(px(DETAIL_PAD))
            .child(body)
            .into_any_element()
    }

    /// The face with the mood switch under it, the name, role and job beside it, and the sections below.
    fn profile(&self, entry: &Entry, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let bot = &entry.bot;
        let id = bot.id.to_string();
        let selector_id = id.clone();
        let ink = Rgba::from(theme.foreground);
        let portrait = face_of(&self.faces, bot).map(|model| {
            let motion = Some((self.runtime.clone(), self.started));
            face(self.faces.clone(), model, self.mood, motion, PROFILE_FACE, ink)
        });
        let moods = Mood::ALL.into_iter().map(|m| Segment::new(mood_words(m)).debug_name(mood_name(m)));
        let selected = Mood::ALL.iter().position(|m| *m == self.mood).unwrap_or(0);
        let switch = cx.weak_entity();
        let mood = Segmented::new("bot-mood", moods, selected)
            .on_change(move |i, _, cx| drop(switch.update(cx, |page, cx| page.set_mood(Mood::ALL[i], cx))));
        let head = div()
            .flex()
            .items_start()
            .gap(px(24.))
            .child(div().flex().flex_col().items_center().gap(px(12.)).children(portrait).child(mood))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pt(px(8.))
                    .child(crate::control::marked_named(
                        format!("bot-profile-{id}"),
                        div()
                            .debug_selector(move || format!("bot-profile-{selector_id}"))
                            .text_size(TextSize::Xl.font_size())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(SharedString::from(bot.name.clone())),
                    ))
                    .child(div().text_size(TextSize::Base.font_size()).text_color(theme.muted_foreground).child(SharedString::from(bot.role.clone())))
                    .child(div().mt(px(10.)).text_size(TextSize::Sm.font_size()).child(SharedString::from(bot.job.clone()))),
            );
        let skills: Vec<AnyElement> = bot.skills.iter().map(|s| Badge::new(SharedString::from(s.clone())).into_any_element()).collect();
        let tools: Vec<SharedString> =
            bot.tools.iter().map(|t| format!("{} · {}", t.connector, access_words(t.access)).into()).collect();
        let notes: Vec<SharedString> = entry.notes.iter().map(|n| SharedString::from(n.text.clone())).collect();
        div()
            .debug_selector(|| "bot-profile".into())
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(head)
            .child(section("Harness", cx).child(line(format!("{} · {}", harness_words(bot.harness), provider_words(&bot.provider)))))
            .child(section("Voice", cx).child(line(voice_words(bot.voice))))
            .child(section("Skills", cx).child(if skills.is_empty() {
                faint("No skills.", &theme)
            } else {
                div().flex().flex_wrap().gap(px(6.)).children(skills).into_any_element()
            }))
            .child(section("Connectors", cx).child(if tools.is_empty() {
                faint("No connectors.", &theme)
            } else {
                div().flex().flex_col().gap(px(4.)).children(tools.into_iter().map(line)).into_any_element()
            }))
            .child(section("Notes", cx).debug_selector(|| "bot-notes".into()).child(if notes.is_empty() {
                faint(NO_NOTES, &theme)
            } else {
                div().flex().flex_col().gap(px(6.)).children(notes.into_iter().map(line)).into_any_element()
            }))
            .child(faint(format!("Version {}", bot.version), &theme))
            .into_any_element()
    }
}

/// The name a test finds a mood segment by.
fn mood_name(mood: Mood) -> &'static str {
    match mood {
        Mood::Idle => "bot-mood-idle",
        Mood::Thinking => "bot-mood-thinking",
        Mood::Working => "bot-mood-working",
        Mood::Done => "bot-mood-done",
        Mood::Needs => "bot-mood-needs",
        Mood::Stuck => "bot-mood-stuck",
    }
}

/// A section of the profile: its heading in the muted ink, and its content under it.
fn section(heading: &'static str, cx: &Context<BotsPage>) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(TextSize::Xs.font_size()).text_color(cx.theme().muted_foreground).child(heading))
}

fn line(words: impl Into<SharedString>) -> AnyElement {
    div().text_size(TextSize::Sm.font_size()).child(words.into()).into_any_element()
}

fn faint(words: impl Into<SharedString>, theme: &atelier_ui::Theme) -> AnyElement {
    div().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child(words.into()).into_any_element()
}

/// Shown on its own (a module's view), the page is its profile.
impl Render for BotsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.main(cx)
    }
}
