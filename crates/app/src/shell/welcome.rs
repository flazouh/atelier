//! The welcome page: the first thing a new reader sees. It covers the whole window from the first start until the
//! reader presses its button, and the settings keep that they did, so it shows once.

use atelier_ui::WelcomePage;
use gpui_kit::{AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Styled, WindowControlArea, div};

use super::{Shell, helpers::settings_path, types::TITLE_BAR};
use atelier_ui::scale::px;

/// Whether the reader is owed the welcome page: the settings do not say they pressed its button.
pub(super) fn owed(saved: &atelier_settings::Settings) -> bool {
    saved.welcomed != Some(true)
}

impl Shell {
    /// Shows the welcome page to a reader who has not seen it. The app calls it once, as the window opens.
    pub fn welcome_at_start(&mut self, saved: &atelier_settings::Settings, cx: &mut Context<Self>) {
        self.welcome = owed(saved);
        cx.notify();
    }

    /// Whether the welcome page covers the window, for the control socket.
    pub fn welcome_shown(&self) -> bool {
        self.welcome
    }

    /// The reader pressed the page's button: the page goes for good, and the settings keep that.
    pub fn dismiss_welcome(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.welcome) {
            return;
        }
        // Off the UI thread. If the write fails the page is gone for this run and shows again at the next start.
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.welcomed = Some(true)) {
                    eprintln!("could not keep that the welcome page was seen: {error}");
                }
            })
            .detach();
        }
        cx.notify();
    }

    /// The page over the whole window, the title bar too. A strip at its top still drags the window, as the title
    /// bar under it does.
    pub(super) fn welcome_page(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.welcome {
            return None;
        }
        let this = cx.entity().downgrade();
        let page = WelcomePage::new("welcome").on_continue(move |_, cx| drop(this.update(cx, |shell, cx| shell.dismiss_welcome(cx))));
        Some(
            div()
                .debug_selector(|| "welcome".into())
                .absolute()
                .inset_0()
                .occlude()
                .child(page)
                .child(div().id("welcome-drag").absolute().top_0().left_0().right_0().h(px(TITLE_BAR)).window_control_area(WindowControlArea::Drag))
                .into_any_element(),
        )
    }
}
