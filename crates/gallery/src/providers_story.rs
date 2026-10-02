//! A mock of choosing and setting up Claude Code's providers: the composer's agent and provider button group, the
//! provider menu open, a session row naming a provider that is not the default, and the Providers section of
//! Settings. Nothing here is wired; it is for judging the design.
use atelier_agents::{coding_agents::CodingAgent, labs::Lab};
use atelier_ui::{
    Badge, BrandMark, Button, ButtonGroup, ButtonSize, ButtonVariant, Select, Tone,
    icon::{Icon, IconName},
    menu::{Choice, Entry, Menu, MenuItem},
    theme::{ActiveTheme, Theme, radius},
    typography::{MONO_FONT_FAMILY, TextSize},
};
use gpui_kit::{App, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder, px};

use super::section;

fn mark_label(mark: Option<BrandMark>, words: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .when_some(mark, |d, mark| d.child(gpui_kit::img(mark.for_theme(theme.appearance)).flex_none().size(px(14.))))
        .child(SharedString::from(words.to_string()))
}

/// The control that sits where the agent picker is today: the agent, and the provider when it has more than one.
fn picker(id: &str, agent: CodingAgent, provider: Option<&str>, theme: &Theme) -> impl IntoElement {
    let agent_button = Button::new(SharedString::from(format!("{id}-agent")))
        .content(mark_label(agent.mark(), agent.name(), theme))
        .trailing_icon(IconName::ChevronDown);
    match provider {
        Some(provider) => ButtonGroup::new(SharedString::from(format!("{id}-group")))
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Md)
            .child(agent_button)
            .child(Button::new(SharedString::from(format!("{id}-provider"))).label(provider.to_string()).trailing_icon(IconName::ChevronDown))
            .into_any_element(),
        None => agent_button.variant(ButtonVariant::Secondary).size(ButtonSize::Md).into_any_element(),
    }
}

/// A composer as a new session shows it, with the picker above the box.
fn composer(id: &str, agent: CodingAgent, provider: Option<&str>, model: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .w(px(520.))
        .child(div().flex().child(picker(id, agent, provider, theme)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(12.))
                .rounded(radius::card())
                .bg(theme.card)
                .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child(format!("Ask {}", agent.name())))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(TextSize::Xs.font_size())
                        .text_color(theme.muted_foreground)
                        .child(Button::new(SharedString::from(format!("{id}-model"))).label(model.to_string()).trailing_icon(IconName::ChevronDown).variant(ButtonVariant::Ghost))
                        .child(div().flex_1())
                        .child(Button::new(SharedString::from(format!("{id}-send"))).icon(IconName::ArrowUp).size(ButtonSize::Icon)),
                ),
        )
}

fn provider_menu() -> impl IntoElement {
    let entries: Vec<Entry> = vec![
        Entry::Label("Claude accounts".into()),
        MenuItem::new("Max").description("~/.claude · signed in").choice(Choice::Selected(true)).into(),
        MenuItem::new("Max · work").description("~/.claude-work · signed in").choice(Choice::Selected(false)).into(),
        MenuItem::new("Team").description("~/.claude-team · not signed in").disabled(true).into(),
        Entry::Separator,
        Entry::Label("API".into()),
        MenuItem::new("OpenRouter").description("Key in the keychain · any model").choice(Choice::Selected(false)).into(),
        Entry::Separator,
        MenuItem::new("Manage providers…").icon(IconName::Settings).into(),
    ];
    div().w(px(260.)).child(Menu::new("provider-menu", entries))
}

/// A sidebar session row, with the provider named quietly when it is not the default.
fn session_row(title: &str, provider: Option<&str>, time: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .w(px(300.))
        .h(px(30.))
        .px(px(10.))
        .rounded(radius::md())
        .text_size(TextSize::Sm.font_size())
        .child(div().flex_1().min_w_0().truncate().child(title.to_string()))
        .when_some(provider, |d, p| d.child(div().flex_none().text_size(TextSize::Xs.font_size()).text_color(theme.faint()).child(p.to_string())))
        .child(div().flex_none().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(time.to_string()))
}

/// The sections of the real Settings pane, with Providers where it would go.
const SECTIONS: [&str; 7] = ["Appearance", "Sidebar", "Agents", "Providers", "Dictation", "Tasks", "Keys"];

/// The narrowest a provider card gets before the row wraps: two to a row in the pane's column.
const CARD_MIN_W: f32 = 220.;

fn mark_tile(mark: Option<BrandMark>, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.))
        .rounded(radius::md())
        .bg(theme.card_strong)
        .map(|d| match mark {
            Some(mark) => d.child(gpui_kit::img(mark.for_theme(theme.appearance)).size(px(20.))),
            None => d.child(Icon::new(IconName::Add).size(px(18.)).color(theme.muted_foreground)),
        })
}

fn dot(colour: gpui_kit::Hsla) -> impl IntoElement {
    div().flex_none().size(px(6.)).rounded_full().bg(colour)
}

fn card_head(mark: Option<BrandMark>, name: &str, detail: &str, mono: bool, badge: Option<Badge>, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(mark_tile(mark, theme))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(div().text_size(TextSize::Sm.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(name.to_string()))
                .child(
                    div()
                        .truncate()
                        .text_size(TextSize::Xs.font_size())
                        .text_color(theme.muted_foreground)
                        .when(mono, |d| d.font_family(MONO_FONT_FAMILY))
                        .child(detail.to_string()),
                ),
        )
        .children(badge)
}

fn card(id: &str, theme: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = theme.muted_foreground.opacity(0.35);
    div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(px(14.))
        .rounded(radius::card())
        .bg(theme.card)
        .border_1()
        .border_color(theme.divider)
        .hover(move |s| s.border_color(hover))
}

fn account_card(id: &str, name: &str, folder: &str, signed_in: bool, default: bool, theme: &Theme) -> impl IntoElement {
    let footer = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .text_size(TextSize::Xs.font_size())
        .text_color(theme.muted_foreground)
        .child(dot(if signed_in { theme.success } else { theme.warning }))
        .child(if signed_in { "Signed in" } else { "Not signed in" })
        .child(div().flex_1())
        .child(if signed_in {
            Button::new(SharedString::from(format!("{id}-more"))).icon(IconName::MoreHoriz).size(ButtonSize::Icon).variant(ButtonVariant::Ghost).into_any_element()
        } else {
            Button::new(SharedString::from(format!("{id}-sign-in"))).label("Sign in").variant(ButtonVariant::Secondary).into_any_element()
        });
    card(id, theme)
        .flex_1().min_w(px(CARD_MIN_W))
        .child(card_head(CodingAgent::ClaudeCode.mark(), name, folder, true, default.then(|| Badge::new("Default")), theme))
        .child(footer)
}

fn add_account_card(theme: &Theme) -> impl IntoElement {
    let hover = theme.card;
    div()
        .id("add-account")
        .flex()
        .flex_col()
        .justify_center()
        .flex_1().min_w(px(CARD_MIN_W))
        .p(px(14.))
        .rounded(radius::card())
        .border_1()
        .border_dashed()
        .border_color(theme.divider)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(card_head(None, "Add a Claude account", "Signs in to its own folder", false, None, theme))
}

fn openrouter_card(theme: &Theme) -> impl IntoElement {
    let key = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(34.))
        .px(px(10.))
        .rounded(radius::md())
        .bg(theme.background)
        .border_1()
        .border_color(theme.divider)
        .child(Icon::new(IconName::Lock).size(px(14.)).color(theme.muted_foreground))
        .child(div().flex_1().min_w_0().truncate().text_size(TextSize::Xs.font_size()).font_family(MONO_FONT_FAMILY).text_color(theme.foreground).child("sk-or-v1-••••••••••••••••3f2a"))
        .child(div().flex_none().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("System keychain"));
    let actions = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(Button::new("or-test").label("Test").variant(ButtonVariant::Secondary))
        .child(Button::new("or-replace").label("Replace key").variant(ButtonVariant::Ghost))
        .child(div().flex_1())
        .child(Button::new("or-site").label("openrouter.ai").trailing_icon(IconName::OpenInNew).variant(ButtonVariant::Ghost));
    card("openrouter", theme)
        .w_full()
        .child(card_head(Lab::OpenRouter.mark(), "OpenRouter", "Any model on OpenRouter, paid per token", false, Some(Badge::new("Key works").tone(Tone::Success)), theme))
        .child(key)
        .child(actions)
}

fn group(title: &str, gist: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(28.))
        .pb(px(12.))
        .child(div().text_size(TextSize::Sm.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(title.to_string()))
        .child(div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(gist.to_string()))
}

fn nav(theme: &Theme) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(180.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .px(px(12.))
        .pt(px(40.))
        .child(div().px(px(8.)).pb(px(12.)).text_size(TextSize::Lg.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child("Settings"))
        .children(SECTIONS.into_iter().map(|words| {
            let front = words == "Providers";
            div()
                .flex()
                .items_center()
                .h(px(32.))
                .px(px(8.))
                .rounded(radius::md())
                .text_size(TextSize::Sm.font_size())
                .text_color(if front { theme.foreground } else { theme.muted_foreground })
                .when(front, |d| d.bg(theme.card_strong).font_weight(FontWeight::MEDIUM))
                .child(words)
        }))
}

/// The Settings pane on its Providers section, at the real pane's widths.
fn settings(theme: &Theme) -> impl IntoElement {
    let accounts = div()
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .child(account_card("acc-max", "Max", "~/.claude", true, true, theme))
        .child(account_card("acc-work", "Max · work", "~/.claude-work", true, false, theme))
        .child(account_card("acc-team", "Team", "~/.claude-team", false, false, theme))
        .child(add_account_card(theme));
    let default = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .min_h(px(44.))
        .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child("Provider for new sessions"))
        .child(div().w(px(220.)).child(Select::new("default-provider", ["Max", "Max · work", "OpenRouter"]).selected(Some(0))));
    let body = div()
        .flex_1()
        .min_w_0()
        .max_w(px(640.))
        .px(px(32.))
        .py(px(40.))
        .flex()
        .flex_col()
        .child(div().text_size(TextSize::Xl.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child("Providers"))
        .child(div().pt(px(4.)).text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child("Where Claude Code gets its model. A session picks one beside its agent."))
        .child(group("Claude accounts", "A subscription, signed in with claude auth login. Its sessions count against that plan.", theme))
        .child(accounts)
        .child(group("API keys", "Claude Code calls the provider instead of Anthropic, with the provider's models.", theme))
        .child(openrouter_card(theme))
        .child(group("Defaults", "A session keeps the provider it started with.", theme))
        .child(default);
    div()
        .flex()
        .w_full()
        .max_w(px(860.))
        .rounded(radius::card())
        .overflow_hidden()
        .bg(theme.background)
        .border_1()
        .border_color(theme.divider)
        .child(nav(theme))
        .child(body)
}

pub fn providers_story(cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    div()
        .child(section(
            "Composer, new session: the agent and its provider as one button group",
            div()
                .flex()
                .flex_col()
                .gap(px(20.))
                .child(composer("p-max", CodingAgent::ClaudeCode, Some("Max"), "Opus", &theme))
                .child(composer("p-or", CodingAgent::ClaudeCode, Some("OpenRouter"), "anthropic/claude-opus-5.5", &theme))
                .child(composer("p-cursor", CodingAgent::Cursor, None, "Auto", &theme)),
        ))
        .child(section("The provider menu, open", provider_menu()))
        .child(section(
            "Sidebar rows: the provider shows only when it is not the default",
            div()
                .flex()
                .flex_col()
                .child(session_row("Fix the rail", None, "2m", &theme))
                .child(session_row("Slice 4: safe delete", Some("work"), "1h", &theme))
                .child(session_row("Try GPT on the parser", Some("OpenRouter"), "3h", &theme)),
        ))
}

pub fn settings_story(cx: &App) -> impl IntoElement {
    settings(cx.theme())
}
