//! The Accounts section: the outside services a person connected for their tasks. Linear with an API key, kept in the
//! system keychain and never in the settings file, and GitHub Issues of one repository over the `gh` login, which holds
//! no key. A row says how its service stands: working, not signed in, offline. The providers themselves are built by
//! [`crate::accounts`], which this page asks to start over after each change.
use atelier_capabilities::CapError;
use atelier_settings::{AccountsSaved, GithubIssuesSaved, LinearSaved, secrets::LINEAR_KEY};
use atelier_ui::{
    ActiveTheme, Badge, Button, ButtonVariant, TextInput, Tone,
    icon::{Icon, IconName},
    scale::px,
    theme::{Theme, radius},
    typography::{MONO_FONT_FAMILY, TextSize},
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, IntoElement, ParentElement, SharedString,
    Styled, Task, Window, component::input::InputState, div, prelude::FluentBuilder,
};

use super::{
    SettingsPane,
    helpers::save,
    providers::{card, card_head_of, group, icon_tile, tail},
};
use crate::{
    accounts::{self, Kind, Row, Rows},
    capability_hub::CapabilityHub,
    control::marked,
};

const LINEAR_PLACEHOLDER: &str = "lin_api_…";
const LINEAR_KEY_START: &str = "lin_api_";
const LINEAR_KEYS_SITE: &str = "https://linear.app/settings/account/security";
const REPO_PLACEHOLDER: &str = "owner/repo";

/// What the last Test or Save found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Checked {
    #[default]
    Unchecked,
    Checking,
    /// Holds the name of the person it signed in as.
    Works(String),
    /// Holds the reason, in plain words.
    Refused(String),
}

impl Checked {
    fn of(outcome: Result<String, String>) -> Self {
        outcome.map_or_else(Checked::Refused, Checked::Works)
    }
}

#[derive(Default)]
pub(crate) struct AccountsPage {
    /// What the settings file keeps of the accounts, as this page changes it.
    pub(crate) saved: AccountsSaved,
    /// `owner/repo` of the open project, when its remote is on GitHub.
    pub(crate) project_repo: Option<String>,
    /// The last characters of the kept Linear key; `None` when no key is kept.
    pub(crate) linear_tail: Option<String>,
    pub(crate) linear_check: Checked,
    pub(crate) github_check: Checked,
    /// Why the keychain could not be read or written.
    pub(crate) problem: Option<String>,
    /// The field the Linear key is typed in, while one is entered.
    linear_field: Option<Entity<InputState>>,
    /// The field the repository is typed in. Made when the section is first drawn.
    github_field: Option<Entity<InputState>>,
    _reading: Option<Task<()>>,
    _linear: Option<Task<()>>,
    _github: Option<Task<()>>,
}

impl AccountsPage {
    pub(crate) fn new(saved: AccountsSaved) -> Self {
        Self {
            saved,
            ..Self::default()
        }
    }
}

/// The Linear key as the page shows a kept one.
fn masked(tail: &str) -> String {
    format!("{LINEAR_KEY_START}••••••••••••{tail}")
}

impl SettingsPane {
    /// Says which GitHub repository the open project has, if any, for "Use this project's repository".
    pub(crate) fn set_project_repo(&mut self, repo: Option<String>, cx: &mut Context<Self>) {
        self.accounts.project_repo = repo;
        cx.notify();
    }

    /// Reads whether a Linear key is kept, off the UI thread. The keychain is asked only when Linear is connected.
    pub(crate) fn read_accounts(&mut self, cx: &mut Context<Self>) {
        self.accounts.problem = None;
        if self.accounts.saved.linear.is_none() {
            self.accounts.linear_tail = None;
            return;
        }
        let secrets = accounts::services(cx).secrets;
        let reading = cx.background_spawn(async move { secrets.read(LINEAR_KEY) });
        self.accounts._reading = Some(cx.spawn(async move |this, cx| {
            let key = reading.await;
            _ = this.update(cx, |pane, cx| {
                match key {
                    Ok(key) => pane.accounts.linear_tail = key.as_deref().map(tail),
                    Err(why) => {
                        pane.accounts.problem =
                            Some(format!("The keychain could not be read: {why}"))
                    }
                }
                cx.notify();
            });
        }));
    }

    /// Asks Linear who `typed` is, or the kept key when none is typed.
    pub(crate) fn test_linear(&mut self, typed: Option<String>, cx: &mut Context<Self>) {
        let services = accounts::services(cx);
        let typed = typed
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty());
        self.accounts.linear_check = Checked::Checking;
        self.accounts.problem = None;
        let work = cx.background_spawn(async move {
            let key = match typed {
                Some(key) => key,
                None => match services.secrets.read(LINEAR_KEY) {
                    Ok(Some(key)) => key,
                    Ok(None) => return Err("No key is kept.".to_string()),
                    Err(why) => return Err(format!("The keychain could not be read: {why}")),
                },
            };
            accounts::connect_linear(&services, &key)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Linear, &e))
        });
        self.accounts._linear = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.linear_check = Checked::of(outcome);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Keeps `key` in the keychain, checks it, and builds the provider. The settings file learns only that Linear is
    /// connected, and the name of the person.
    pub(crate) fn save_linear(&mut self, key: String, cx: &mut Context<Self>) {
        let key = key.trim().to_string();
        if key.is_empty() {
            return;
        }
        let services = accounts::services(cx);
        self.accounts.linear_check = Checked::Checking;
        self.accounts.problem = None;
        self.accounts.linear_field = None;
        let work = cx.background_spawn(async move {
            services
                .secrets
                .write(LINEAR_KEY, &key)
                .map_err(|e| format!("The keychain could not keep the key: {e}"))?;
            let found = accounts::connect_linear(&services, &key)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Linear, &e));
            Ok::<_, String>((tail(&key), found))
        });
        self.accounts._linear = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                match outcome {
                    Ok((tail, found)) => {
                        pane.accounts.linear_tail = Some(tail);
                        pane.accounts.saved.linear = Some(LinearSaved {
                            person: found.clone().ok(),
                        });
                        pane.accounts.linear_check = Checked::of(found);
                        pane.accounts_changed(cx);
                    }
                    Err(why) => {
                        pane.accounts.linear_check = Checked::Unchecked;
                        pane.accounts.problem = Some(why);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_linear(&mut self, cx: &mut Context<Self>) {
        self.accounts.problem = None;
        let secrets = accounts::services(cx).secrets;
        let work = cx.background_spawn(async move { secrets.forget(LINEAR_KEY) });
        self.accounts._linear = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                match outcome {
                    Ok(()) => {
                        pane.accounts.linear_tail = None;
                        pane.accounts.linear_check = Checked::Unchecked;
                        pane.accounts.saved.linear = None;
                        pane.accounts_changed(cx);
                    }
                    Err(why) => {
                        pane.accounts.problem =
                            Some(format!("The keychain could not forget the key: {why}"))
                    }
                }
                cx.notify();
            });
        }));
    }

    /// Opens `repo` over the `gh` login and reads one issue.
    pub(crate) fn test_github(&mut self, repo: String, cx: &mut Context<Self>) {
        let services = accounts::services(cx);
        self.accounts.github_check = Checked::Checking;
        let work = cx.background_spawn(async move {
            accounts::connect_github(&services, &repo)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::GithubIssues, &e))
        });
        self.accounts._github = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.github_check = Checked::of(outcome);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Connects GitHub Issues of `repo`: checks it, keeps the repository and the person's name in the settings, and
    /// builds the provider.
    pub(crate) fn save_github(&mut self, repo: String, cx: &mut Context<Self>) {
        let repo = repo.trim().to_string();
        if repo.is_empty() {
            return;
        }
        let services = accounts::services(cx);
        self.accounts.github_check = Checked::Checking;
        let kept = repo.clone();
        let work = cx.background_spawn(async move {
            accounts::connect_github(&services, &kept)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::GithubIssues, &e))
        });
        self.accounts._github = Some(cx.spawn(async move |this, cx| {
            let found = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.saved.github_issues = Some(GithubIssuesSaved {
                    repo,
                    person: found.clone().ok(),
                });
                pane.accounts.github_check = Checked::of(found);
                pane.accounts_changed(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_github(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.accounts.saved.github_issues = None;
        self.accounts.github_check = Checked::Unchecked;
        if let Some(field) = &self.accounts.github_field {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        self.accounts_changed(cx);
        cx.notify();
    }

    /// Fills the repository field with the open project's repository.
    pub(crate) fn use_project_repo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(repo), Some(field)) = (
            self.accounts.project_repo.clone(),
            self.accounts.github_field.clone(),
        ) else {
            return;
        };
        field.update(cx, |field, cx| field.set_value(repo, window, cx));
        cx.notify();
    }

    /// What is saved changed: keeps it in the settings file, and has the providers built again.
    fn accounts_changed(&mut self, cx: &mut Context<Self>) {
        let saved = self.accounts.saved.clone();
        let kept = saved.clone();
        save(cx, move |s| s.accounts = kept);
        accounts::refresh(saved, cx);
    }

    fn open_linear_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(LINEAR_PLACEHOLDER)
                .masked(true)
        });
        field.read(cx).focus_handle(cx).focus(window, cx);
        self.accounts.linear_field = Some(field);
        cx.notify();
    }

    pub(super) fn accounts_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        if self.accounts.github_field.is_none() {
            let repo = self
                .accounts
                .saved
                .github_issues
                .as_ref()
                .map(|g| g.repo.clone())
                .unwrap_or_default();
            let field = cx.new(|cx| {
                let mut field = InputState::new(window, cx).placeholder(REPO_PLACEHOLDER);
                field.set_value(repo, window, cx);
                field
            });
            // The button that offers the project's repository comes and goes with what is typed.
            cx.observe(&field, |_, _, cx| cx.notify()).detach();
            self.accounts.github_field = Some(field);
        }
        let rows = cx
            .try_global::<CapabilityHub>()
            .map(CapabilityHub::rows)
            .unwrap_or_default();
        let linear = self.linear_card(&rows, &theme, cx);
        let github = self.github_card(&rows, &theme, cx);
        div()
            .flex()
            .flex_col()
            .when_some(self.accounts.problem.clone(), |d, problem| {
                d.child(div().pb(px(8.)).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(problem))
            })
            .child(group("Tasks", "Where your tasks live besides this project. A connected account shows in the Tasks screen and reaches your agents.", &theme))
            .child(div().flex().flex_col().gap(px(12.)).child(linear).child(github))
            .into_any_element()
    }

    fn linear_card(&self, rows: &Rows, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let page = &self.accounts;
        let row = &rows.linear;
        let detail = match row {
            Row::Connected(name) => format!("Signed in as {name}"),
            _ => page
                .saved
                .linear
                .as_ref()
                .and_then(|l| l.person.clone())
                .map_or_else(
                    || "Tasks from your Linear workspace".to_string(),
                    |p| format!("Signed in as {p}"),
                ),
        };
        let body = match (&page.linear_field, &page.linear_tail) {
            (Some(field), kept) => {
                let (test, save, cancel) = (this.clone(), this.clone(), this.clone());
                let (typed_for_test, typed_for_save) = (field.clone(), field.clone());
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        TextInput::new("linear-key", field)
                            .surface(theme.background)
                            .left_icon(IconName::Lock),
                    )
                    .children(check_line(&page.linear_check, theme))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(marked(
                                "linear-test",
                                Button::new("linear-test")
                                    .label("Test")
                                    .variant(ButtonVariant::Secondary)
                                    .disabled(page.linear_check == Checked::Checking)
                                    .on_click(move |_, _, cx| {
                                        let typed = typed_for_test.read(cx).value().to_string();
                                        test.update(cx, |p, cx| p.test_linear(Some(typed), cx))
                                            .ok();
                                    }),
                            ))
                            .child(marked(
                                "linear-save",
                                Button::new("linear-save")
                                    .label("Save in the keychain")
                                    .variant(ButtonVariant::Secondary)
                                    .on_click(move |_, _, cx| {
                                        let typed = typed_for_save.read(cx).value().to_string();
                                        save.update(cx, |p, cx| p.save_linear(typed, cx)).ok();
                                    }),
                            ))
                            .when(kept.is_some(), |d| {
                                d.child(marked(
                                    "linear-cancel",
                                    Button::new("linear-cancel")
                                        .label("Cancel")
                                        .variant(ButtonVariant::Ghost)
                                        .on_click(move |_, _, cx| {
                                            cancel
                                                .update(cx, |p, cx| {
                                                    p.accounts.linear_field = None;
                                                    cx.notify();
                                                })
                                                .ok();
                                        }),
                                ))
                            }),
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
                            .child(
                                Icon::new(IconName::Lock)
                                    .size(px(14.))
                                    .color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(TextSize::Xs.font_size())
                                    .font_family(MONO_FONT_FAMILY)
                                    .text_color(theme.foreground)
                                    .child(masked(tail)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(TextSize::Xs.font_size())
                                    .text_color(theme.muted_foreground)
                                    .child("System keychain"),
                            ),
                    )
                    .children(row_note(Kind::Linear, row, theme))
                    .children(check_line(&page.linear_check, theme))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(marked(
                                "linear-test",
                                Button::new("linear-test")
                                    .label("Test")
                                    .variant(ButtonVariant::Secondary)
                                    .disabled(page.linear_check == Checked::Checking)
                                    .on_click(move |_, _, cx| {
                                        test.update(cx, |p, cx| p.test_linear(None, cx)).ok();
                                    }),
                            ))
                            .child(marked(
                                "linear-replace",
                                Button::new("linear-replace")
                                    .label("Replace key")
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(move |_, window, cx| {
                                        replace
                                            .update(cx, |p, cx| p.open_linear_field(window, cx))
                                            .ok();
                                    }),
                            ))
                            .child(div().flex_1())
                            .child(marked(
                                "linear-forget",
                                Button::new("linear-forget")
                                    .label("Forget")
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(move |_, _, cx| {
                                        forget.update(cx, |p, cx| p.forget_linear(cx)).ok();
                                    }),
                            )),
                    )
                    .into_any_element()
            }
            (None, None) => {
                let add = this.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(marked(
                        "linear-add",
                        Button::new("linear-add")
                            .label("Add a key")
                            .icon(IconName::Add)
                            .variant(ButtonVariant::Secondary)
                            .on_click(move |_, window, cx| {
                                add.update(cx, |p, cx| p.open_linear_field(window, cx)).ok();
                            }),
                    ))
                    .child(div().flex_1())
                    .child(
                        Button::new("linear-site")
                            .label("Get a key")
                            .trailing_icon(IconName::OpenInNew)
                            .variant(ButtonVariant::Ghost)
                            .on_click(|_, _, cx| {
                                cx.open_url(LINEAR_KEYS_SITE);
                            }),
                    )
                    .into_any_element()
            }
        };
        card("linear", theme)
            .w_full()
            .child(card_head_of(
                icon_tile(IconName::Checklist, theme),
                "Linear",
                &detail,
                false,
                badge_of(row, page.saved.linear.is_some()),
                theme,
            ))
            .child(body)
            .into_any_element()
    }

    fn github_card(&self, rows: &Rows, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let page = &self.accounts;
        let row = &rows.github;
        let Some(field) = page.github_field.clone() else {
            return div().into_any_element();
        };
        let typed = field.read(cx).value().trim().to_string();
        let saved_repo = page.saved.github_issues.as_ref().map(|g| g.repo.clone());
        let detail = match (row, &saved_repo) {
            (Row::Connected(name), Some(repo)) => format!("{repo} · {name}"),
            (_, Some(repo)) => repo.clone(),
            _ => "Issues of one repository, over your gh login".to_string(),
        };
        // Offered while the field holds something else, so a person who typed their own repository is not nagged.
        let offer = page.project_repo.clone().filter(|repo| *repo != typed);
        let (test, save, use_repo, forget) =
            (this.clone(), this.clone(), this.clone(), this.clone());
        let (for_test, for_save) = (field.clone(), field.clone());
        card("github-issues", theme)
            .w_full()
            .child(card_head_of(icon_tile(IconName::GitBranch, theme), "GitHub Issues", &detail, false, badge_of(row, saved_repo.is_some()), theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(TextInput::new("github-repo", &field).surface(theme.background))
                    .child(div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child("Uses the account you signed in with gh auth login. No key is kept here."))
                    .children(row_note(Kind::GithubIssues, row, theme))
                    .children(check_line(&page.github_check, theme))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(marked(
                                "github-test",
                                Button::new("github-test").label("Test").variant(ButtonVariant::Secondary).disabled(page.github_check == Checked::Checking).on_click(move |_, _, cx| {
                                    let repo = for_test.read(cx).value().to_string();
                                    test.update(cx, |p, cx| p.test_github(repo, cx)).ok();
                                }),
                            ))
                            .child(marked(
                                "github-save",
                                Button::new("github-save").label("Save").variant(ButtonVariant::Secondary).on_click(move |_, _, cx| {
                                    let repo = for_save.read(cx).value().to_string();
                                    save.update(cx, |p, cx| p.save_github(repo, cx)).ok();
                                }),
                            ))
                            .when(offer.is_some(), |d| {
                                d.child(marked(
                                    "github-use-project",
                                    Button::new("github-use-project").label("Use this project's repository").variant(ButtonVariant::Ghost).on_click(move |_, window, cx| {
                                        use_repo.update(cx, |p, cx| p.use_project_repo(window, cx)).ok();
                                    }),
                                ))
                            })
                            .child(div().flex_1())
                            .when(saved_repo.is_some(), |d| {
                                d.child(marked(
                                    "github-forget",
                                    Button::new("github-forget").label("Forget").variant(ButtonVariant::Ghost).on_click(move |_, window, cx| {
                                        forget.update(cx, |p, cx| p.forget_github(window, cx)).ok();
                                    }),
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }
}

/// The badge at a card's head: how its service stands.
fn badge_of(row: &Row, connected: bool) -> Option<Badge> {
    Some(match row {
        Row::Off if connected => return None,
        Row::Off => Badge::new("Not connected"),
        Row::Checking => Badge::new("Checking…"),
        Row::Connected(_) => Badge::new("Connected").tone(Tone::Success),
        Row::NotSignedIn => Badge::new("Not signed in").tone(Tone::Warning),
        Row::Offline => Badge::new("Offline").tone(Tone::Warning),
        Row::Failed(_) => Badge::new("Problem").tone(Tone::Danger),
    })
}

/// The words under a row that is not working, so a badge is never the only thing said.
fn row_note(kind: Kind, row: &Row, theme: &Theme) -> Option<AnyElement> {
    let words = match row {
        Row::NotSignedIn => accounts::plain_words(kind, &CapError::NotSignedIn),
        Row::Offline => accounts::plain_words(kind, &CapError::Offline),
        Row::Failed(words) => words.clone(),
        Row::Off | Row::Checking | Row::Connected(_) => return None,
    };
    Some(
        div()
            .text_size(TextSize::Xs.font_size())
            .text_color(theme.danger)
            .child(SharedString::from(words))
            .into_any_element(),
    )
}

/// What a Test or a Save found, in one line.
fn check_line(check: &Checked, theme: &Theme) -> Option<AnyElement> {
    let (words, colour) = match check {
        Checked::Unchecked => return None,
        Checked::Checking => ("Checking…".to_string(), theme.muted_foreground),
        Checked::Works(name) => (format!("Works. Signed in as {name}."), theme.success),
        Checked::Refused(why) => (why.clone(), theme.danger),
    };
    Some(
        div()
            .text_size(TextSize::Xs.font_size())
            .text_color(colour)
            .child(SharedString::from(words))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests;
