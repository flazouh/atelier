use atelier_capabilities::{
    Actor, CapError, CapResult, Operation, Ref,
    tasks::{self as v1, TasksProvider},
};
use atelier_ui::{
    Button, ButtonSize, ButtonVariant, Select,
    theme::Theme,
    typography::TextSize,
};
use atelier_ui::scale::px;
use gpui_kit::{
    App, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
};

use super::super::source::{Choice, Vocabulary};
use super::structs::Loaded;
use super::types::{Problem, Reaction};

/// How the screen answers an error: offline and a wait are banners over the tasks that stay, signed out is an
/// empty state, a call that is not offered loses its control, and anything else is one line.
pub fn react(error: &CapError) -> Reaction {
    match error {
        CapError::Offline => Reaction::Raise(Problem::Offline),
        CapError::RateLimited { retry_after_ms } => Reaction::Raise(Problem::Wait(*retry_after_ms)),
        CapError::NotSignedIn => Reaction::Raise(Problem::SignedOut),
        CapError::Unsupported { .. } => Reaction::Hide,
        other => Reaction::Line(other.to_string()),
    }
}

/// A list a provider may not have: a call it does not list, or answers with `Unsupported`, gives none.
fn optional<T>(listed: bool, call: impl FnOnce() -> CapResult<Vec<T>>) -> CapResult<Vec<T>> {
    if !listed {
        return Ok(Vec::new());
    }
    match call() {
        Err(CapError::Unsupported { .. }) => Ok(Vec::new()),
        other => other,
    }
}

/// The first `pages` pages of the provider's tasks, with its statuses, labels and projects. Blocks.
pub fn read(provider: &dyn TasksProvider, pages: usize) -> CapResult<Loaded> {
    let caps = provider.capabilities();
    let mut query = v1::Query { limit: caps.limits.page_max, ..v1::Query::default() };
    let (mut tasks, mut next) = (Vec::new(), None);
    for _ in 0..pages.max(1) {
        let page = provider.list(&query)?;
        tasks.extend(page.items);
        next = page.next_cursor;
        match &next {
            Some(cursor) => query.cursor = Some(cursor.clone()),
            None => break,
        }
    }
    let labels = optional(caps.can(Operation::Labels), || provider.labels())?;
    let projects = optional(caps.can(Operation::Projects), || provider.projects())?;
    let statuses = optional(caps.can(Operation::Statuses), || provider.statuses())?;
    let vocab = Vocabulary::new(provider.provider(), provider.account(), caps, statuses, labels, projects);
    Ok(Loaded { tasks, next, vocab })
}

/// The page that follows `cursor`. Blocks.
pub fn read_more(provider: &dyn TasksProvider, cursor: &str) -> CapResult<v1::Page<v1::Task>> {
    let query = v1::Query { limit: provider.capabilities().limits.page_max, cursor: Some(cursor.to_string()), ..v1::Query::default() };
    provider.list(&query)
}

/// Every line of a task's activity, oldest first. Blocks.
pub fn read_activity(provider: &dyn TasksProvider, task: &Ref) -> CapResult<Vec<v1::Activity>> {
    let mut lines = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = provider.activity(task, cursor.as_deref())?;
        lines.extend(page.items);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(lines),
        }
    }
}

/// Changes a task by `patch`. A patch holds only the fields that change, so when the task changed meanwhile it is
/// sent again against the version now: two quick changes of one task do not refuse each other. Blocks.
pub fn apply(provider: &dyn TasksProvider, task: &Ref, patch: &v1::Patch, version: &str, by: &Actor) -> CapResult<v1::Task> {
    match provider.update(task, patch, version, by) {
        Err(CapError::Conflict { current }) => {
            let now = serde_json::from_value::<v1::Task>(current).map_err(|_| CapError::invalid("version"))?;
            provider.update(task, patch, &now.version, by)
        }
        other => other,
    }
}

/// The switcher of the header: a small select with each provider and account. The caller shows it only when
/// there is more than one.
pub fn switcher(choices: &[Choice], selected: Option<usize>, on_change: impl Fn(usize, &mut Window, &mut App) + 'static) -> gpui_kit::AnyElement {
    div().debug_selector(|| "tasks-provider".into()).flex_none().child(
        Select::new("tasks-provider-select", choices.iter().map(Choice::words))
            .compact(true)
            .selected(selected)
            .panel_width(px(220.))
            .on_change(on_change),
    )
    .into_any_element()
}

/// The banner of a problem the tasks wait out: offline has Retry, a wait says how long. `None` for the problem that
/// takes the whole screen.
pub fn banner(problem: Problem, retry: impl Fn(&mut Window, &mut App) + 'static, theme: &Theme) -> Option<gpui_kit::AnyElement> {
    let words = match problem {
        Problem::Offline => "There is no connection.".to_string(),
        Problem::Wait(ms) => format!("Too many requests. Try again in {} s.", ms.div_ceil(1000)),
        Problem::SignedOut => return None,
    };
    let row = div()
        .id("tasks-banner")
        .debug_selector(|| "tasks-banner".into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .pb(px(8.))
        .child(div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(words));
    let row = if problem == Problem::Offline {
        row.child(
            Button::new("tasks-retry")
                .debug_name("tasks-retry")
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

/// The empty state of a provider the reader is not signed in to, with a way to the Accounts section of Settings.
pub fn signed_out(provider: &str, open_accounts: impl Fn(&mut Window, &mut App) + 'static, theme: &Theme) -> gpui_kit::AnyElement {
    div()
        .id("tasks-signed-out")
        .debug_selector(|| "tasks-signed-out".into())
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .px(px(24.))
        .child(div().text_color(theme.muted_foreground).child(format!("Sign in to {provider} to see its tasks.")))
        .child(
            Button::new("tasks-sign-in")
                .debug_name("tasks-sign-in")
                .label("Open Accounts")
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .on_click(move |_, window, cx| open_accounts(window, cx)),
        )
        .into_any_element()
}

/// The row under the tasks that reads the next page.
pub fn load_more(loading: bool, load: impl Fn(&mut Window, &mut App) + 'static) -> impl IntoElement {
    div().id("tasks-load-more-row").flex().flex_none().justify_center().py(px(6.)).child(
        Button::new("tasks-load-more")
            .debug_name("tasks-load-more")
            .label(if loading { "Loading…" } else { "Load more" })
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Sm)
            .on_click(move |_, window, cx| load(window, cx)),
    )
}
