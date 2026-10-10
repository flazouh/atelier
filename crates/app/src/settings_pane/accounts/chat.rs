//! The Chat group of the Accounts section: Slack and Discord, each through the reader's own command line tool. The tool
//! keeps the login, so nothing here is a secret: the settings file keeps only the names, the channels, the switches and
//! the paths. Test asks the tool who is signed in and changes nothing; Save keeps the facts and has the app build the
//! provider (see [`crate::accounts::refresh`]); Forget takes them away.
use atelier_settings::{DiscordSaved, SlackSaved};
use atelier_ui::{icon::IconName, scale::px, theme::Theme};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Task, Window,
    component::input::InputState, div,
};

use super::{
    AccountsPage, Checked, badge_of, check_line,
    parts::{Buttons, actions, labelled, note, switch_row, typed},
    row_note,
};
use crate::{
    accounts::{self, Kind, Row, Rows, split_list},
    settings_pane::{
        SettingsPane,
        providers::{card, card_head_of, icon_tile},
    },
};

const SLACK: Buttons = Buttons {
    test: "slack-test",
    save: "slack-save",
    forget: "slack-forget",
};
const DISCORD: Buttons = Buttons {
    test: "discord-test",
    save: "discord-save",
    forget: "discord-forget",
};
/// Said under Allow sending, from `docs/capabilities/discord-notes.md`.
const DISCORD_RISK: &str = "Discord does not allow a program to act on a user login, and an account that sends this way can be banned. Reading is the default.";

/// What the Chat cards hold while the page is open.
#[derive(Default)]
pub(crate) struct ChatPage {
    pub(crate) slack_check: Checked,
    pub(crate) discord_check: Checked,
    /// The two switches as the reader set them: Save keeps them.
    pub(crate) discord_dms: bool,
    pub(crate) discord_writes: bool,
    fields: Option<ChatFields>,
    _slack: Option<Task<()>>,
    _discord: Option<Task<()>>,
}

/// The fields the cards are typed in. Made when the section is first drawn.
struct ChatFields {
    slack_workspace: Entity<InputState>,
    slack_channels: Entity<InputState>,
    slack_program: Entity<InputState>,
    discord_server: Entity<InputState>,
    discord_program: Entity<InputState>,
}

fn field(
    placeholder: &str,
    value: &str,
    window: &mut Window,
    cx: &mut Context<SettingsPane>,
) -> Entity<InputState> {
    let (placeholder, value) = (placeholder.to_string(), value.to_string());
    cx.new(|cx| {
        let mut field = InputState::new(window, cx).placeholder(placeholder);
        field.set_value(value, window, cx);
        field
    })
}

impl SettingsPane {
    /// Makes the fields, filled with what is saved, the first time the section is drawn.
    pub(super) fn ensure_chat_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.accounts.chat.fields.is_some() {
            return;
        }
        let slack = self.accounts.saved.slack.clone().unwrap_or_default();
        let discord = self.accounts.saved.discord.clone().unwrap_or_default();
        self.accounts.chat.discord_dms = discord.include_dms;
        self.accounts.chat.discord_writes = discord.allow_writes;
        self.accounts.chat.fields = Some(ChatFields {
            slack_workspace: field("acme", &slack.workspace, window, cx),
            slack_channels: field(
                "general, random, C0123ABCD",
                &slack.channels.join(", "),
                window,
                cx,
            ),
            slack_program: field(
                "slackcli (looked up on your PATH)",
                &slack.program,
                window,
                cx,
            ),
            discord_server: field(
                "Server name or id (empty: direct messages only)",
                &discord.server,
                window,
                cx,
            ),
            discord_program: field(
                "discordcli (looked up on your PATH)",
                &discord.program,
                window,
                cx,
            ),
        });
    }

    /// A field of the Chat cards by its name, for a test that types into it.
    #[cfg(test)]
    pub(crate) fn chat_field(&self, name: &str) -> Entity<InputState> {
        let fields = self
            .accounts
            .chat
            .fields
            .as_ref()
            .expect("the section was drawn");
        match name {
            "slack-workspace" => &fields.slack_workspace,
            "slack-channels" => &fields.slack_channels,
            "slack-program" => &fields.slack_program,
            "discord-server" => &fields.discord_server,
            "discord-program" => &fields.discord_program,
            other => panic!("no field {other}"),
        }
        .clone()
    }

    pub(crate) fn slack_draft(&self, cx: &Context<Self>) -> Option<SlackSaved> {
        let fields = self.accounts.chat.fields.as_ref()?;
        Some(SlackSaved {
            workspace: typed(&fields.slack_workspace, cx),
            channels: split_list(&typed(&fields.slack_channels, cx)),
            program: typed(&fields.slack_program, cx),
            person: None,
        })
    }

    pub(crate) fn discord_draft(&self, cx: &Context<Self>) -> Option<DiscordSaved> {
        let fields = self.accounts.chat.fields.as_ref()?;
        Some(DiscordSaved {
            server: typed(&fields.discord_server, cx),
            include_dms: self.accounts.chat.discord_dms,
            allow_writes: self.accounts.chat.discord_writes,
            program: typed(&fields.discord_program, cx),
            person: None,
        })
    }

    /// Asks `slackcli` who is signed in, with what is typed. Keeps nothing.
    pub(crate) fn test_slack(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.slack_draft(cx) else {
            return;
        };
        let services = accounts::services(cx);
        self.accounts.chat.slack_check = Checked::Checking;
        let work = cx.background_spawn(async move {
            accounts::connect_slack(&services, &draft)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Slack, &e))
        });
        self.accounts.chat._slack = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.chat.slack_check = Checked::of(outcome);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Keeps the Slack facts in the settings and has the provider built. A tool that is missing or logged out is kept as
    /// well: the row says why, and the account works once the reader fixes it.
    pub(crate) fn save_slack(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.slack_draft(cx) else {
            return;
        };
        let services = accounts::services(cx);
        self.accounts.chat.slack_check = Checked::Checking;
        let asked = draft.clone();
        let work = cx.background_spawn(async move {
            accounts::connect_slack(&services, &asked)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Slack, &e))
        });
        self.accounts.chat._slack = Some(cx.spawn(async move |this, cx| {
            let found = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.saved.slack = Some(SlackSaved {
                    person: found.clone().ok(),
                    ..draft
                });
                pane.accounts.chat.slack_check = Checked::of(found);
                pane.accounts_changed(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_slack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.accounts.saved.slack = None;
        self.accounts.chat.slack_check = Checked::Unchecked;
        if let Some(fields) = &self.accounts.chat.fields {
            for field in [
                &fields.slack_workspace,
                &fields.slack_channels,
                &fields.slack_program,
            ] {
                field.update(cx, |field, cx| field.set_value("", window, cx));
            }
        }
        self.accounts_changed(cx);
        cx.notify();
    }

    /// Asks `discordcli` who is signed in, with what is typed. Keeps nothing.
    pub(crate) fn test_discord(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.discord_draft(cx) else {
            return;
        };
        let services = accounts::services(cx);
        self.accounts.chat.discord_check = Checked::Checking;
        let work = cx.background_spawn(async move {
            accounts::connect_discord(&services, &draft)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Discord, &e))
        });
        self.accounts.chat._discord = Some(cx.spawn(async move |this, cx| {
            let outcome = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.chat.discord_check = Checked::of(outcome);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Keeps the Discord facts, the two switches among them, and has the provider built.
    pub(crate) fn save_discord(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.discord_draft(cx) else {
            return;
        };
        let services = accounts::services(cx);
        self.accounts.chat.discord_check = Checked::Checking;
        let asked = draft.clone();
        let work = cx.background_spawn(async move {
            accounts::connect_discord(&services, &asked)
                .map(|(_, name)| name)
                .map_err(|e| accounts::plain_words(Kind::Discord, &e))
        });
        self.accounts.chat._discord = Some(cx.spawn(async move |this, cx| {
            let found = work.await;
            _ = this.update(cx, |pane, cx| {
                pane.accounts.saved.discord = Some(DiscordSaved {
                    person: found.clone().ok(),
                    ..draft
                });
                pane.accounts.chat.discord_check = Checked::of(found);
                pane.accounts_changed(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn forget_discord(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.accounts.saved.discord = None;
        self.accounts.chat.discord_check = Checked::Unchecked;
        // A forgotten account starts again with sending off.
        self.accounts.chat.discord_dms = false;
        self.accounts.chat.discord_writes = false;
        if let Some(fields) = &self.accounts.chat.fields {
            for field in [&fields.discord_server, &fields.discord_program] {
                field.update(cx, |field, cx| field.set_value("", window, cx));
            }
        }
        self.accounts_changed(cx);
        cx.notify();
    }

    pub(super) fn slack_card(
        &self,
        rows: &Rows,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page: &AccountsPage = &self.accounts;
        let Some(fields) = &page.chat.fields else {
            return div().into_any_element();
        };
        let this = cx.entity().downgrade();
        let row = &rows.slack;
        let saved = page.saved.slack.as_ref();
        let detail = match (row, saved) {
            (Row::Connected(name), Some(slack)) => format!("{} · {name}", slack.workspace),
            (_, Some(slack)) => slack.workspace.clone(),
            _ => "Channels of your Slack workspace, over slackcli".to_string(),
        };
        let (test, save, forget) = (this.clone(), this.clone(), this);
        card("slack", theme)
            .w_full()
            .child(card_head_of(icon_tile(IconName::ChatBubble, theme), "Slack", &detail, false, badge_of(row, saved.is_some()), theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(labelled("Workspace", "slack-workspace", &fields.slack_workspace, theme))
                    .child(labelled("Channels", "slack-channels", &fields.slack_channels, theme))
                    .child(note("Names or ids, separated by commas: the channels your agents reach. slackcli has no channel list, so name the ones you want.", theme))
                    .child(labelled("Path of slackcli", "slack-program", &fields.slack_program, theme))
                    .child(note("Uses the login of slackcli (run slackcli login in a terminal). No key is kept here.", theme))
                    .children(row_note(Kind::Slack, row, &page.chat.slack_check, theme))
                    .children(check_line(&page.chat.slack_check, theme))
                    .child(actions(
                        SLACK,
                        &page.chat.slack_check,
                        saved.is_some(),
                        move |_, cx| {
                            test.update(cx, |p, cx| p.test_slack(cx)).ok();
                        },
                        move |_, cx| {
                            save.update(cx, |p, cx| p.save_slack(cx)).ok();
                        },
                        move |window, cx| {
                            forget.update(cx, |p, cx| p.forget_slack(window, cx)).ok();
                        },
                    )),
            )
            .into_any_element()
    }

    pub(super) fn discord_card(
        &self,
        rows: &Rows,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page: &AccountsPage = &self.accounts;
        let Some(fields) = &page.chat.fields else {
            return div().into_any_element();
        };
        let this = cx.entity().downgrade();
        let row = &rows.discord;
        let saved = page.saved.discord.as_ref();
        let detail = match (row, saved) {
            (Row::Connected(name), Some(discord)) if !discord.server.is_empty() => {
                format!("{} · {name}", discord.server)
            }
            (Row::Connected(name), Some(_)) => format!("Direct messages · {name}"),
            (_, Some(discord)) if !discord.server.is_empty() => discord.server.clone(),
            _ => "A server of yours, over discordcli".to_string(),
        };
        let (dms, writes) = (this.clone(), this.clone());
        let (test, save, forget) = (this.clone(), this.clone(), this);
        card("discord", theme)
            .w_full()
            .child(card_head_of(icon_tile(IconName::Forum, theme), "Discord", &detail, false, badge_of(row, saved.is_some()), theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(labelled("Server", "discord-server", &fields.discord_server, theme))
                    .child(switch_row("discord-dms", "Include direct messages", page.chat.discord_dms, None, theme, move |on, _, cx| {
                        dms.update(cx, |p, cx| {
                            p.accounts.chat.discord_dms = on;
                            cx.notify();
                        })
                        .ok();
                    }))
                    .child(switch_row("discord-writes", "Allow sending", page.chat.discord_writes, Some((DISCORD_RISK, theme.muted_foreground)), theme, move |on, _, cx| {
                        writes
                            .update(cx, |p, cx| {
                                p.accounts.chat.discord_writes = on;
                                cx.notify();
                            })
                            .ok();
                    }))
                    .child(labelled("Path of discordcli", "discord-program", &fields.discord_program, theme))
                    .child(note("Uses the login of discordcli (run discordcli login in a terminal). No key is kept here.", theme))
                    .children(row_note(Kind::Discord, row, &page.chat.discord_check, theme))
                    .children(check_line(&page.chat.discord_check, theme))
                    .child(actions(
                        DISCORD,
                        &page.chat.discord_check,
                        saved.is_some(),
                        move |_, cx| {
                            test.update(cx, |p, cx| p.test_discord(cx)).ok();
                        },
                        move |_, cx| {
                            save.update(cx, |p, cx| p.save_discord(cx)).ok();
                        },
                        move |window, cx| {
                            forget.update(cx, |p, cx| p.forget_discord(window, cx)).ok();
                        },
                    )),
            )
            .into_any_element()
    }
}
