use std::rc::Rc;

use atelier_capabilities::{
    CapError, CapResult, Ref,
    messaging::{ChannelQuery, Formatting, MessagingCapabilities, MessagingProvider},
};
use atelier_ui::{
    Button, ButtonSize, ButtonVariant, scale::px, theme::Theme, typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, InteractiveElement, IntoElement, ListOffset, ListState, ParentElement, Styled,
    Window, div,
};

use super::super::map::{Line, Row, line_of, row_of};
use super::types::{CHANNEL_PAGES, PAGE_FALLBACK, PAGE_MOST, Problem, Reaction, THREAD_PAGES};

/// How the screen answers an error: offline and a wait are banners over what stays, signed out is an empty state, a call
/// that is not offered loses its control, and anything else is one line.
pub fn react(error: &CapError) -> Reaction {
    match error {
        CapError::Offline => Reaction::Raise(Problem::Offline),
        CapError::RateLimited { retry_after_ms } => Reaction::Raise(Problem::Wait(*retry_after_ms)),
        CapError::NotSignedIn => Reaction::Raise(Problem::SignedOut),
        CapError::Unsupported { .. } => Reaction::Hide,
        other => Reaction::Line(other.to_string()),
    }
}

/// How many messages a page asks for: what the provider allows, within what a screen wants.
pub(super) fn page_of(caps: &MessagingCapabilities) -> u32 {
    caps.limits
        .page_max
        .unwrap_or(PAGE_FALLBACK)
        .clamp(1, PAGE_MOST)
}

/// What one reading of a channel's history brings: the lines, oldest first, and the cursor of the page before them.
pub(super) struct Fetched {
    pub lines: Vec<Line>,
    pub next: Option<String>,
}

/// What reading an account's channels brings.
pub(super) struct Channels {
    pub caps: MessagingCapabilities,
    pub rows: Vec<Row>,
}

/// The channels of the account, and what it can do. Blocks.
pub(super) fn read_channels(provider: &dyn MessagingProvider) -> CapResult<Channels> {
    let caps = provider.capabilities();
    let mut query = ChannelQuery {
        limit: caps.limits.page_max,
        ..ChannelQuery::default()
    };
    let mut rows = Vec::new();
    for _ in 0..CHANNEL_PAGES {
        let page = provider.channels(&query)?;
        rows.extend(page.items.iter().map(row_of));
        match page.next_cursor {
            Some(cursor) => query.cursor = Some(cursor),
            None => break,
        }
    }
    Ok(Channels { caps, rows })
}

/// The first `pages` pages of a channel's history, as lines with the oldest first. Blocks.
pub(super) fn read_history(
    provider: &dyn MessagingProvider,
    channel: &Ref,
    pages: usize,
    now: u64,
) -> CapResult<Fetched> {
    let caps = provider.capabilities();
    let (limit, mut cursor, mut newest_first) = (page_of(&caps), None::<String>, Vec::new());
    for _ in 0..pages.max(1) {
        let page = provider.history(channel, cursor.as_deref(), Some(limit))?;
        newest_first.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(Fetched {
        lines: lines_of(newest_first.iter().rev(), caps.formatting, now),
        next: cursor,
    })
}

/// The page of history that follows `cursor`, going back in time. Blocks.
pub(super) fn read_older(
    provider: &dyn MessagingProvider,
    channel: &Ref,
    cursor: &str,
    now: u64,
) -> CapResult<Fetched> {
    let caps = provider.capabilities();
    let page = provider.history(channel, Some(cursor), Some(page_of(&caps)))?;
    Ok(Fetched {
        lines: lines_of(page.items.iter().rev(), caps.formatting, now),
        next: page.next_cursor,
    })
}

/// A thread: the root first, then its replies. Blocks.
pub(super) fn read_thread(
    provider: &dyn MessagingProvider,
    root: &Ref,
    now: u64,
) -> CapResult<Fetched> {
    let formatting = provider.capabilities().formatting;
    let (mut messages, mut cursor) = (Vec::new(), None::<String>);
    for _ in 0..THREAD_PAGES {
        let page = provider.thread(root, cursor.as_deref())?;
        messages.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(Fetched {
        lines: lines_of(messages.iter(), formatting, now),
        next: cursor,
    })
}

fn lines_of<'a>(
    messages: impl Iterator<Item = &'a atelier_capabilities::messaging::Message>,
    formatting: Formatting,
    now: u64,
) -> Vec<Line> {
    messages.map(|m| line_of(m, formatting, now)).collect()
}

/// Puts `next` where `current` is, and tells the list only what changed: the rows from the first that differs to the last
/// that differs. A row before the part that changes stays where it is, and when the change is above the first row in view
/// (an older page came in) the view is moved by as many rows as were added, so the reader keeps the place they were at.
pub(super) fn apply(current: &mut Rc<Vec<Line>>, list: &ListState, next: Vec<Line>) {
    let old = Rc::clone(current);
    let before = old.iter().zip(&next).take_while(|(a, b)| a == b).count();
    let room = old.len().min(next.len()) - before;
    let after = old
        .iter()
        .rev()
        .zip(next.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let removed = before..old.len() - after;
    let added = next.len() - before - after;
    if removed.is_empty() && added == 0 {
        return;
    }
    let place = list.logical_scroll_top();
    let (from, to) = (removed.len(), added);
    list.splice(removed.clone(), added);
    if !list.is_following_tail() && removed.end <= place.item_ix && from != to {
        let item_ix = (place.item_ix + to).saturating_sub(from);
        list.scroll_to(ListOffset {
            item_ix,
            offset_in_item: place.offset_in_item,
        });
    }
    *current = Rc::new(next);
}

/// The banner of a problem the pane waits out: offline has Retry, a wait says how long. `None` for the problem that takes the
/// whole pane.
pub(super) fn banner(
    problem: Problem,
    retry: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> Option<AnyElement> {
    let words = match problem {
        Problem::Offline => "There is no connection.".to_string(),
        Problem::Wait(ms) => format!("Too many requests. Try again in {} s.", ms.div_ceil(1000)),
        Problem::SignedOut => return None,
    };
    let row = div()
        .id("messages-banner")
        .debug_selector(|| "messages-banner".into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .pb(px(8.))
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(words),
        );
    let row = if problem == Problem::Offline {
        row.child(
            Button::new("messages-retry")
                .debug_name("messages-retry")
                .label("Retry")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| retry(window, cx)),
        )
    } else {
        row
    };
    Some(row.into_any_element())
}

/// The empty state of an account the reader is not signed in to, with a way to Settings.
pub(super) fn signed_out(
    account: &str,
    open_settings: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    state(
        "messages-signed-out",
        format!("Sign in to {account} to see its messages."),
        "messages-sign-in",
        "Open settings",
        open_settings,
        theme,
    )
}

/// The empty state of an app with no chat account.
pub(super) fn no_account(
    open_settings: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    state(
        "messages-empty",
        "No chat account is connected.".to_string(),
        "messages-open-settings",
        "Open settings",
        open_settings,
        theme,
    )
}

fn state(
    name: &'static str,
    words: String,
    button: &'static str,
    label: &'static str,
    press: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    div()
        .id(name)
        .debug_selector(move || name.into())
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .px(px(24.))
        .child(div().text_color(theme.muted_foreground).child(words))
        .child(
            Button::new(button)
                .debug_name(button)
                .label(label)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| press(window, cx)),
        )
        .into_any_element()
}

/// One muted line in the middle of the pane: what it waits for, or what went wrong.
pub(super) fn say(words: gpui_kit::SharedString, theme: &Theme) -> AnyElement {
    div()
        .debug_selector(|| "messages-line".into())
        .flex_1()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(24.))
        .text_color(theme.muted_foreground)
        .child(words)
        .into_any_element()
}

/// The row over the history that reads the page before it.
pub(super) fn load_older(
    loading: bool,
    load: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id("messages-load-older-row")
        .flex()
        .flex_none()
        .justify_center()
        .py(px(6.))
        .child(
            Button::new("messages-load-older")
                .debug_name("messages-load-older")
                .label(if loading { "Loading…" } else { "Load older" })
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| load(window, cx)),
        )
        .into_any_element()
}
