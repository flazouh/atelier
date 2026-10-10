# Slots, v1 (draft for review)
Status: draft. The first two slots exist in code (`crates/app/src/slots`, and `crates/plugin` for the view). The others are planned and have no code yet.
## 1. Why slots
The base app is raw: a window, a sidebar, a session, a bar at the foot. What else the reader sees is **contributed**.
A capability (`tasks`, `mail`, `messaging`) or a plugin adds its pieces to **named slots**, and the base app draws
whatever the slots hold. The base app does not know any contribution by name.
The test of a slot is removal: take one module's registration out, and what it added is gone from the app. The shell,
the bar and the rail do not change.
Words used here:
- **Slot**: a named place in the app that accepts contributions of one kind.
- **Contribution**: one piece a module gives to a slot.
- **Registry**: the `Slots` global. It holds the contributions of every slot.
## 2. The slots as they stand
### 2.1 Status bar card
The bar at the foot of the window has three columns, as the panes above it: left (under the sidebar), middle, and right
(under the right pane). The bar draws one panel under each column. A card is a piece of one of them.
A contribution gives:
| Field | Meaning |
|---|---|
| `id` | Names the card. A card added with the id of another replaces it. |
| `column` | `Left`, `Middle` or `Right`. |
| `order` | A number. Cards of a column stand from the lowest to the highest. The same number goes by `id`. |
| `visible` | A check on the bar's numbers (`Vitals`), made on each draw. A card that is not visible is not drawn. Default: always. |
| `render` | Draws the card. It gets the bar's id, the numbers and the host, and returns the parts to put in its column. |
Rules the bar keeps:
- The left panel is as wide as the sidebar and stands under it. With the sidebar hidden, its cards lead the middle panel.
- The right panel is as wide as the right pane and stands under it, while the pane is shown and the column has a card.
  Otherwise its cards end the middle panel.
- The gaps between panels come from `atelier_ui::panel_layout::GAP` and follow the zoom. A contribution never sets one.
Built-in cards:
| id | Column | Order | Module | Visible when |
|---|---|---|---|---|
| `version` | Left | 0 | `changelog` | always. A press opens the changelog. |
| `work` | Middle | 10 | `vitals` | an agent session waits on the reader. |
| `usage` | Middle | 20 | `usage_view` | always. The chips of the 5 hour and weekly limits. A press opens the `usage` view. |
| `system` | Right | 0 | `vitals` | the machine was sampled. CPU and memory. |
### 2.2 Plugin view
A view a plugin adds: an entry on the left rail, and the page it opens. `docs/plugins.md` is the guide.
A contribution gives:
| Field | Meaning |
|---|---|
| `id` | Names the view. A view added with the id of another replaces it. |
| `icon` | The icon of the entry on the left rail. |
| `label` | The name of the entry. |
| `order` | The place of the entry on the rail, lowest first, after the app's own three. The same number goes by `id`. |
| the page | Makes the page of the view. It gets the host. The shell keeps one page per view, and draws its sidebar and its main area. |
Anything may open a registered view by id with `Host::open_view`.
Built-in views: `bots` (module `bots_view`, order 90) and `usage` (module `usage_view`, order 100). The usage chips in
the status bar also open `usage`.
The rail draws its three own views (Sessions, Tasks, Code) and then every registered view.
## 3. Planned slots
Each row says what a contribution gives. None has code yet.
| Slot | Where | A contribution gives |
|---|---|---|
| Session header badge | The head of a session, beside the title. | `id`, `order`, `visible` (a check on the session), a render function that returns a small badge, and what a press does. |
| Composer chip | The row under the text box of a session. | `id`, `order`, `visible` (a check on the session and the draft), a render function that returns a chip, and what a press does. |
| Slash command | The `/` list of the composer. | `name`, a one line `description`, an `args` hint, and what running it does with the session and the draft. |
| Tool result card | A tool call in a conversation. | The tool names it matches, and a render function that takes the call and its result and returns a card. Without a match, the app draws the generic row. A capability card (`card-schema-v1.md`) is the first contribution. |
| Settings section | The list of sections in Settings. | `id`, `title`, `order`, `icon`, a render function for the pane, and the keys it reads and writes in the settings file. |
| Menu entries | The ⋯ menus of a project and a session, and the main menu. | The `menu` they go in, `id`, `order`, `label`, an optional icon and shortcut, `visible`, and what a press does. |
All of them follow the rules of section 4: an `id`, replace by `id`, `order`, and `visible`.
## 4. How a capability or a plugin registers
1. A module that adds a card has one function, `register(slots: &mut Slots)`, which calls `add_card` once per card.
   A plugin implements `atelier_plugin::Plugin` and adds its views there. Render functions and pages use only the
   host they are given: the numbers, and the few calls of `Host` (`open_view`, `show_changelog`). A contribution
   never takes a handle to the shell.
2. `slots::builtin()` calls `register` of each module and `plug` for each plugin. That is the only list. It runs once,
   at startup, before the window opens, and its result is set as a global.
3. A second contribution with an existing `id` replaces the first. A later module can therefore replace a built-in one.
4. A module that is not in the list adds nothing, and the opening of a view it would have registered does nothing.
   The tests hold this for the usage module: without its registration the bar has no chips and `open_view("usage")`
   opens nothing.
Plugins that load at run time are not part of v1, so a plugin is a Rust crate that `builtin()` lists
(`docs/plugins.md`).
## 5. Open questions
- Should `visible` take more than the bar's numbers? A session badge needs the session. Each slot will pass its own
  state, as the bar passes `Vitals`.
- Should the rail's three own views move onto the registry in a later step?
- Where does a plugin declare the capabilities it needs, so a card for a capability that is not there is not drawn?
