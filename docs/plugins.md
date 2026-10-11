# Plugins

A plugin is one Rust crate that adds a view to atelier. A view is an entry on the left rail and what a press on it
shows: a sidebar and a main area. The app draws what the plugins registered and knows no plugin by name. Take a
plugin's line out of the registry, and its entry and its view are gone. Nothing else changes.

A plugin is compiled into the app. There is no loading at run time, no manifest file and no second process.

## What a plugin depends on

One crate: `atelier-plugin` (`crates/plugin`). It holds the four things below, and it exports `atelier_ui` (the
design system) and `gpui_kit` (GPUI) again. It does not depend on the app, so a plugin builds and tests without it.

```toml
[dependencies]
atelier-plugin.workspace = true
```

| Name | What it is |
|---|---|
| `Plugin` | The trait a plugin implements. Its one job: register what the plugin adds. |
| `PluginView` | What a plugin registers: an id, an icon, a label, an order, and how to make the page. |
| `PluginPage` | The trait the page implements: the sidebar, the main area, and a call when the view comes in front again. |
| `Host` | The handle the page gets to the app. |

## The plugin

```rust
pub trait Plugin {
    fn register(&self, registry: &mut Registry);
}
```

The app calls `register` once, at startup, before the window opens. The plugin calls `registry.add_view` for each
view it adds.

A `PluginView` has:

| Field | Meaning |
|---|---|
| `id` | Names the view. The settings keep the view in front by it, `Host::open_view` opens it by it, and the control socket answers `view <id>`. A view registered with the id of another replaces it. Do not take a name of the app's own views: `sessions`, `tasks`, `git`, `files`, `pulls`, `history`. |
| `icon` | The icon of the entry on the rail, an `atelier_ui::IconName`. |
| `label` | The name of the entry. |
| `order` | The place of the entry. The rail shows Sessions, Tasks and Code, then the registered views from the lowest order to the highest. The same order goes by id. Bots is 90 and Usage is 100. |
| the page | A function that makes the page. It gets the host and the page's own GPUI context. |

The debug name of the entry is `rail-<id>`. A test and the control socket find the entry by it.

## The page

```rust
pub trait PluginPage: Sized + 'static {
    fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement;
    fn main(&mut self, cx: &mut Context<Self>) -> AnyElement;
    fn in_front_again(&mut self, _cx: &mut Context<Self>) {}
}
```

- The app keeps one page for each view. It makes the page the first time the view is in front, and keeps it until
  the app quits. What the reader chose in the page stays when the reader goes to another view and comes back.
- `sidebar` draws the sidebar of the view. The app puts it under the project switcher.
- `main` draws the main area. The app puts it on a card beside the sidebar. The page scrolls inside the card.
- `in_front_again` tells the page that its view came in front after another one. A page that reads something (a
  folder, a log) reads it again here. The app does not call it the first time: the page reads as it is made.

The page is a GPUI entity. To draw again after a change, it calls `cx.notify()`.

Build the two parts from `atelier_ui` components, and take every colour from the theme.

## The host

`Host` is the only way a page reaches the app. A clone is cheap: the page keeps one and gives one to each handler.

| Call | What it does |
|---|---|
| `host.open_view(id, cx)` | Brings the registered view `id` in front, at the next frame. Nothing happens for an id nobody registered. |
| `host.vitals(cx)` | The numbers of the status bar, as the app last read them: the machine's load, and each provider's use of its plan. |
| `host.start_session_as(bot, window, cx)` | Starts a session that belongs to the bot kept under the id `bot`, in the project in front, and brings the Sessions view in front with it open. Call it from a handler, which has the window. The app says why when none starts. |

A page never takes a handle to the window or to the app's own state. When a plugin needs more from the app, the call
is added to `Host` (and to `AppHost`, the trait the app implements behind it).

## The one registry line

`crates/app/src/slots/helpers.rs` holds the list. A plugin is one line of it:

```rust
pub fn builtin() -> Slots {
    let mut slots = Slots::default();
    // …
    slots.plug(&crate::bots_view::BotsPlugin::for_reader());
    slots
}
```

A plugin in its own crate also needs the crate in the app's `Cargo.toml`. That is all the app has to know.

## A worked example: Bots

The Bots view is the first plugin. It lives in `crates/app/src/bots_view` for now, and it has the shape a crate has.

The plugin registers one view. It carries what the page needs to start (the folder the bots are kept in), and the
page keeps the host: its profile asks the app for a session of the bot with `host.start_session_as`.

```rust
pub struct BotsPlugin {
    root: Option<PathBuf>,
}

impl Plugin for BotsPlugin {
    fn register(&self, registry: &mut Registry) {
        let root = self.root.clone();
        registry.add_view(PluginView::new("bots", IconName::Bot, "Bots", 90, move |host, cx| BotsPage::new(root.clone(), host.clone(), cx)));
    }
}
```

The page holds the state of the view (the bots that were read, the one chosen) and draws the two parts:

```rust
impl PluginPage for BotsPage {
    fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // The title, then one row per bot. A press on a row calls `page.select(id, cx)`, which calls `cx.notify()`.
    }

    fn main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // The profile of the chosen bot, in a `div` that scrolls.
    }

    fn in_front_again(&mut self, cx: &mut Context<Self>) {
        // Reads the folder again, off the UI thread, so an edit on disk shows.
        self.refresh(cx);
    }
}
```

`BotsPage::new` starts the first read of the folder, so the view has its bots the first time it is in front.

Usage is the second plugin (`crates/app/src/usage_view`). Its page keeps the host and reads the providers from
`host.vitals(cx)`. Its chips in the status bar open the view with `open_view("usage")`.

## Testing a plugin

A test registers the plugin and drives the window, as `crates/app/src/shell/tests/plugins.rs` does with a fake one:

1. Add the plugin to the registry: `cx.global_mut::<Slots>().plug(&MyPlugin)`.
2. Press `rail-<id>` and check the debug names the sidebar and the main area draw.
3. Press another entry and check that they are gone.

A page that needs no window is tested in its own crate: `PluginView::open` makes the page with a `Host` over a fake
`AppHost`, as `crates/plugin/src/tests.rs` does.

## What a plugin cannot add yet

- A card in the status bar. The Usage chips are registered by the app's own `usage_view::register`, beside its plugin.
- A sidebar row that looks like the rows of the app's views. Bots uses a helper of the app (`shell::nav_row_marked`).
  A plugin in its own crate needs that row in `atelier_ui` first.
- Sessions, Tasks and Code are not plugins. They are the app's own views.
