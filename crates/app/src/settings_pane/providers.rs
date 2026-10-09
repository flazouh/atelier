//! The Providers section: the agent's accounts on this machine, the OpenRouter key in the keychain, and the
//! provider new sessions start on.
use atelier_agents::{coding_agents::CodingAgent, labs::Lab, session::Account};
use atelier_settings::secrets::OPENROUTER_KEY;
use atelier_ui::{
    ActiveTheme, Badge, BrandMark, Button, ButtonVariant, Select, TextInput, Tone,
    icon::{Icon, IconName},
    scale::px,
    theme::{Theme, radius},
    typography::{MONO_FONT_FAMILY, TextSize},
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement, StatefulInteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    Task, Window, component::input::InputState, div, prelude::FluentBuilder,
};

use crate::providers::{self, Choice, DefaultProvider};
use super::{SettingsPane, helpers::save};
use crate::control::marked;

/// The narrowest a card gets before its row wraps: two to a row in the page's column.
const CARD_MIN_W: f32 = 220.;
const KEY_TAIL: usize = 4;
const KEY_PLACEHOLDER: &str = "sk-or-v1-…";
const ACCOUNT_PLACEHOLDER: &str = "A name for it, such as work";
const OPENROUTER_SITE: &str = "https://openrouter.ai/keys";

/// The OpenRouter key's state, as far as the page knows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum KeyCheck {
    #[default]
    Unchecked,
    Checking,
    Works,
    Refused(String),
}

#[derive(Default)]
pub(crate) struct ProvidersPage {
    /// `None` while they are read.
    pub(crate) accounts: Option<Vec<Account>>,
    /// The last characters of the kept key; `None` when no key is kept.
    pub(crate) key_tail: Option<String>,
    pub(crate) check: KeyCheck,
    /// Why the keychain or the accounts could not be read.
    pub(crate) problem: Option<String>,
    /// The account a browser is open to sign in to.
    pub(crate) signing_in: Option<String>,
    /// The field the key is typed in, while one is entered.
    key_field: Option<Entity<InputState>>,
    /// The field a new account's name is typed in, while one is added.
    name_field: Option<Entity<InputState>>,
    _reading: Option<Task<()>>,
    _checking: Option<Task<()>>,
    _signing: Option<Task<()>>,
}

pub(super) fn tail(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    chars[chars.len().saturating_sub(KEY_TAIL)..].iter().collect()
}

/// A name `~/.claude-<name>` can take: letters, digits, `-` and `_`.
pub(crate) fn account_name_ok(name: &str) -> bool {
    atelier_agents::claude_code::accounts::is_account_name(name) && name != atelier_agents::claude_code::accounts::DEFAULT_ACCOUNT
}

impl SettingsPane {
    /// Reads the accounts and whether a key is kept, off the UI thread.
    pub(crate) fn read_providers(&mut self, cx: &mut Context<Self>) {
        let services = providers::services(cx);
        let reading = cx.background_spawn(async move { ((services.accounts)(), services.secrets.read(OPENROUTER_KEY)) });
        self.providers._reading = Some(cx.spawn(async move |this, cx| {
            let (accounts, key) = reading.await;
            _ = this.update(cx, |pane, cx| {
                let page = &mut pane.providers;
                page.problem = None;
                match accounts {
                    Ok(accounts) => page.accounts = Some(accounts),
                    Err(why) => {
                        page.accounts = Some(Vec::new());
                        page.problem = Some(format!("The accounts could not be read: {why}"));
                    }
                }
                match key {
                    Ok(key) => page.key_tail = key.as_deref().map(tail),
                    Err(why) => page.problem = Some(format!("The keychain could not be read: {why}")),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn choose_default(&mut self, choice: Choice, cx: &mut Context<Self>) {
        let key = choice.key();
        cx.set_global(DefaultProvider(choice));
        save(cx, move |s| s.default_provider = Some(key));
        cx.notify();
    }

    /// Keeps `key` in the keychain, then asks OpenRouter whether it works.
    pub(crate) fn keep_key(&mut self, key: String, cx: &mut Context<Self>) {
        let key = key.trim().to_string();
        if key.is_empty() {
            return;
        }
        let services = providers::services(cx);
        self.providers.check = KeyCheck::Checking;
        self.providers.key_field = None;
        let work = cx.background_spawn(async move {
            services.secrets.write(OPENROUTER_KEY, &key).map_err(|e| format!("The keychain could not keep the key: {e}"))?;
            Ok::<_, String>((tail(&key), (services.check_key)(&key)))
        });
        self.providers._checking = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                let page = &mut pane.providers;
                match outcome {
                    Ok((tail, check)) => {
                        page.key_tail = Some(tail);
                        page.check = check.map_or_else(KeyCheck::Refused, |()| KeyCheck::Works);
                    }
                    Err(why) => {
                        page.check = KeyCheck::Unchecked;
                        page.problem = Some(why);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn test_key(&mut self, cx: &mut Context<Self>) {
        let services = providers::services(cx);
        self.providers.check = KeyCheck::Checking;
        let work = cx.background_spawn(async move {
            match services.secrets.read(OPENROUTER_KEY) {
                Ok(Some(key)) => (services.check_key)(&key),
                Ok(None) => Err("No key is kept".into()),
                Err(why) => Err(format!("The keychain could not be read: {why}")),
            }
        });
        self.providers._checking = Some(cx.spawn(async move |this, cx| {
            let check = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.providers.check = check.map_or_else(KeyCheck::Refused, |()| KeyCheck::Works);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_key(&mut self, cx: &mut Context<Self>) {
        let secrets = providers::secrets(cx);
        if let Err(why) = secrets.forget(OPENROUTER_KEY) {
            self.providers.problem = Some(format!("The keychain could not forget the key: {why}"));
        } else {
            self.providers.key_tail = None;
            self.providers.check = KeyCheck::Unchecked;
            if providers::default_choice(cx) == Choice::OpenRouter {
                self.choose_default(Choice::usual(), cx);
            }
        }
        cx.notify();
    }

    /// Opens the browser to sign in to `name`, and reads the accounts again when it is done.
    pub(crate) fn sign_in(&mut self, name: String, cx: &mut Context<Self>) {
        let Some((backend, host)) = providers::agent_here() else { return };
        let Some(command) = backend.sign_in(&name) else { return };
        self.providers.signing_in = Some(name);
        self.providers.name_field = None;
        let signing = cx.background_spawn(async move { atelier_agents::subprocess::output(host.as_ref(), &command).map(drop).map_err(|e| e.to_string()) });
        self.providers._signing = Some(cx.spawn(async move |this, cx| {
            let outcome = signing.await;
            _ = this.update(cx, |pane, cx| {
                pane.providers.signing_in = None;
                if let Err(why) = outcome {
                    pane.providers.problem = Some(format!("The sign-in did not finish: {why}"));
                }
                pane.read_providers(cx);
            });
        }));
        cx.notify();
    }

    fn open_key_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(KEY_PLACEHOLDER).masked(true));
        let focus = field.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        self.providers.key_field = Some(field);
        cx.notify();
    }

    fn open_name_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(ACCOUNT_PLACEHOLDER));
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        let focus = field.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        self.providers.name_field = Some(field);
        cx.notify();
    }

    pub(super) fn providers_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let this = cx.entity().downgrade();
        let page = &self.providers;
        let accounts = page.accounts.clone().unwrap_or_default();

        let account_cards = accounts.iter().map(|account| {
            let pane = this.clone();
            let name = account.name.clone();
            let signing = page.signing_in.as_deref() == Some(account.name.as_str());
            let default = providers::default_choice(cx) == Choice::Account(account.name.clone());
            let footer = div()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(dot(if account.signed_in { theme.success } else { theme.warning }))
                .child(SharedString::from(match (signing, account.signed_in, &account.email) {
                    (true, _, _) => "Waiting for the browser…".to_string(),
                    (false, true, Some(email)) => email.clone(),
                    (false, true, None) => "Signed in".to_string(),
                    (false, false, _) => "Not signed in".to_string(),
                }))
                .child(div().flex_1())
                .when(!account.signed_in && !signing, |d| {
                    d.child(Button::new(SharedString::from(format!("sign-in-{name}"))).label("Sign in").variant(ButtonVariant::Secondary).on_click(move |_, _, cx| {
                        pane.update(cx, |p, cx| p.sign_in(name.clone(), cx)).ok();
                    }))
                });
            card(&format!("account-{}", account.name), &theme)
                .flex_1()
                .min_w(px(CARD_MIN_W))
                .child(card_head(
                    CodingAgent::ClaudeCode.mark(),
                    &providers::label(&Choice::Account(account.name.clone()), &accounts),
                    &folder_of(&account.name),
                    true,
                    default.then(|| Badge::new("Default")),
                    &theme,
                ))
                .child(footer)
                .into_any_element()
        });

        let add_card = match &page.name_field {
            Some(field) => {
                let (pane, field_value) = (this.clone(), field.clone());
                let typed = field.read(cx).value().trim().to_string();
                let ok = account_name_ok(&typed);
                card("add-account", &theme)
                    .flex_1()
                    .min_w(px(CARD_MIN_W))
                    .child(TextInput::new("account-name", field).surface(theme.card).invalid(!typed.is_empty() && !ok).error("Letters, digits, - and _ only"))
                    .child(
                        div().flex().gap(px(6.)).child(
                            Button::new("add-account-sign-in").label("Sign in").variant(ButtonVariant::Secondary).disabled(!ok).on_click(move |_, _, cx| {
                                let name = field_value.read(cx).value().trim().to_string();
                                pane.update(cx, |p, cx| p.sign_in(name, cx)).ok();
                            }),
                        ),
                    )
                    .into_any_element()
            }
            None => {
                let pane = this.clone();
                let hover = theme.card;
                div()
                    .id("add-account")
                    .debug_selector(|| "add-account".into())
                    .flex()
                    .flex_col()
                    .justify_center()
                    .flex_1()
                    .min_w(px(CARD_MIN_W))
                    .p(px(14.))
                    .rounded(radius::card())
                    .border_1()
                    .border_dashed()
                    .border_color(theme.divider)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(move |_, window, cx| {
                        pane.update(cx, |p, cx| p.open_name_field(window, cx)).ok();
                    })
                    .child(card_head(None, "Add a Claude account", "Signs in to its own folder", false, None, &theme))
                    .into_any_element()
            }
        };

        let accounts_grid = div()
            .flex()
            .flex_wrap()
            .gap(px(12.))
            .when(page.accounts.is_none(), |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("Reading the accounts…")))
            .children(account_cards)
            .child(add_card);

        let openrouter = self.openrouter_card(&theme, cx);

        let mut choices: Vec<Choice> = accounts.iter().filter(|a| a.signed_in).map(|a| Choice::Account(a.name.clone())).collect();
        if page.key_tail.is_some() {
            choices.push(Choice::OpenRouter);
        }
        let current = providers::default_choice(cx);
        if !choices.contains(&current) {
            choices.insert(0, current.clone());
        }
        let words: Vec<String> = choices.iter().map(|c| providers::label(c, &accounts)).collect();
        let pane = this.clone();
        let default_row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .min_h(px(44.))
            .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child("Provider for new sessions"))
            .child(div().w(px(220.)).debug_selector(|| "default-provider".into()).child(
                Select::new("default-provider", words).selected(choices.iter().position(|c| *c == current)).on_change(move |i, _, cx| {
                    let Some(choice) = choices.get(i).cloned() else { return };
                    pane.update(cx, |p, cx| p.choose_default(choice, cx)).ok();
                }),
            ));

        div()
            .flex()
            .flex_col()
            .when_some(page.problem.clone(), |d, problem| {
                d.child(div().pb(px(8.)).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(problem))
            })
            .child(group("Claude accounts", "A subscription, signed in with claude auth login. Its sessions count against that plan.", &theme))
            .child(accounts_grid)
            .child(group("API keys", "Claude Code calls the provider instead of Anthropic, with the provider's models.", &theme))
            .child(openrouter)
            .child(group("Defaults", "A session keeps the provider it started with.", &theme))
            .child(default_row)
            .into_any_element()
    }

    fn openrouter_card(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let page = &self.providers;
        let badge = match (&page.check, &page.key_tail) {
            (KeyCheck::Works, _) => Some(Badge::new("Key works").tone(Tone::Success)),
            (KeyCheck::Refused(_), _) => Some(Badge::new("Key refused").tone(Tone::Danger)),
            (KeyCheck::Checking, _) => Some(Badge::new("Checking…")),
            (KeyCheck::Unchecked, None) => Some(Badge::new("No key").tone(Tone::Warning)),
            (KeyCheck::Unchecked, Some(_)) => None,
        };
        let body = match (&page.key_field, &page.key_tail) {
            (Some(field), _) => {
                let (pane, value) = (this.clone(), field.clone());
                let cancel = this.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(TextInput::new("openrouter-key", field).surface(theme.background).left_icon(IconName::Lock))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(marked("keep-key", Button::new("keep-key").label("Keep in the keychain").variant(ButtonVariant::Secondary).on_click(move |_, _, cx| {
                                let key = value.read(cx).value().to_string();
                                pane.update(cx, |p, cx| p.keep_key(key, cx)).ok();
                            })))
                            .child(marked("cancel-key", Button::new("cancel-key").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                                cancel.update(cx, |p, cx| {
                                    p.providers.key_field = None;
                                    cx.notify();
                                }).ok();
                            }))),
                    )
                    .into_any_element()
            }
            (None, Some(tail)) => {
                let (test, replace, forget) = (this.clone(), this.clone(), this.clone());
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
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
                            .child(div().flex_1().min_w_0().truncate().text_size(TextSize::Xs.font_size()).font_family(MONO_FONT_FAMILY).text_color(theme.foreground).child(format!("sk-or-v1-••••••••••••{tail}")))
                            .child(div().flex_none().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("System keychain")),
                    )
                    .when_some(match &page.check { KeyCheck::Refused(why) => Some(why.clone()), _ => None }, |d, why| {
                        d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(why))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(marked("test-key", Button::new("test-key").label("Test").variant(ButtonVariant::Secondary).disabled(page.check == KeyCheck::Checking).on_click(move |_, _, cx| {
                                test.update(cx, |p, cx| p.test_key(cx)).ok();
                            })))
                            .child(marked("replace-key", Button::new("replace-key").label("Replace key").variant(ButtonVariant::Ghost).on_click(move |_, window, cx| {
                                replace.update(cx, |p, cx| p.open_key_field(window, cx)).ok();
                            })))
                            .child(div().flex_1())
                            .child(marked("forget-key", Button::new("forget-key").label("Forget").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                                forget.update(cx, |p, cx| p.forget_key(cx)).ok();
                            }))),
                    )
                    .into_any_element()
            }
            (None, None) => {
                let add = this.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(marked("add-key", Button::new("add-key").label("Add a key").icon(IconName::Add).variant(ButtonVariant::Secondary).on_click(move |_, window, cx| {
                        add.update(cx, |p, cx| p.open_key_field(window, cx)).ok();
                    })))
                    .child(div().flex_1())
                    .child(Button::new("openrouter-site").label("Get a key").trailing_icon(IconName::OpenInNew).variant(ButtonVariant::Ghost).on_click(|_, _, cx| {
                        cx.open_url(OPENROUTER_SITE);
                    }))
                    .into_any_element()
            }
        };
        card("openrouter", theme)
            .w_full()
            .child(card_head(Lab::OpenRouter.mark(), "OpenRouter", "Any model on OpenRouter, paid per token", false, badge, theme))
            .child(body)
            .into_any_element()
    }
}

fn folder_of(account: &str) -> String {
    if account == atelier_agents::claude_code::accounts::DEFAULT_ACCOUNT { "~/.claude".into() } else { format!("~/.claude-{account}") }
}

/// The square at the left of a card's head, which holds its mark or icon.
fn tile(theme: &Theme) -> gpui_kit::Div {
    div().flex().flex_none().items_center().justify_center().size(px(36.)).rounded(radius::md()).bg(theme.card_strong)
}

/// A tile with a plain icon, for a service that has no brand mark here.
pub(super) fn icon_tile(icon: IconName, theme: &Theme) -> impl IntoElement {
    tile(theme).child(Icon::new(icon).size(px(18.)).color(theme.foreground))
}

fn mark_tile(mark: Option<BrandMark>, theme: &Theme) -> impl IntoElement {
    tile(theme)
        .map(|d| match mark {
            Some(mark) => d.child(gpui_kit::img(mark.for_theme(theme.appearance)).size(px(20.))),
            None => d.child(Icon::new(IconName::Add).size(px(18.)).color(theme.muted_foreground)),
        })
}

pub(super) fn dot(colour: gpui_kit::Hsla) -> impl IntoElement {
    div().flex_none().size(px(6.)).rounded_full().bg(colour)
}

fn card_head(mark: Option<BrandMark>, name: &str, detail: &str, mono: bool, badge: Option<Badge>, theme: &Theme) -> impl IntoElement {
    card_head_of(mark_tile(mark, theme), name, detail, mono, badge, theme)
}

/// A card's head with `tile` at its left.
pub(super) fn card_head_of(tile: impl IntoElement, name: &str, detail: &str, mono: bool, badge: Option<Badge>, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(tile)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(div().text_size(TextSize::Sm.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(name.to_string()))
                .child(div().truncate().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).when(mono, |d| d.font_family(MONO_FONT_FAMILY)).child(detail.to_string())),
        )
        .children(badge)
}

pub(super) fn card(id: &str, theme: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
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

pub(super) fn group(title: &str, gist: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(24.))
        .pb(px(12.))
        .child(div().text_size(TextSize::Sm.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(title.to_string()))
        .child(div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(gist.to_string()))
}

#[cfg(test)]
mod tests;
