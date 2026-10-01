# The sidebar and the agent panels

Parts in `crates/beui` that show many projects, the sessions under each, and many agent panels at once.
beui names no agent and fetches nothing: every part takes plain data, and the app decides what goes in
a panel.

## Data

- `ProjectData { id, name, location, connection, branch, sessions }`. `Location` is `Local` or
  `Ssh { host }`. `Connection` is `Connected`, `Connecting`, `Reconnecting` or `Offline`.
- `SessionData { id, title, look, status, active_at }`. `look` is an `AgentLook`: the agent's mark as data.
- `SessionStatus` is set by the app from the neutral session events:

| Status | Mark | Words | Title |
| --- | --- | --- | --- |
| `Working` | the agent's mark, animating | the time since it last did anything | ink |
| `NeedsYou(Approval \| Question)` | the warning tone | "Needs approval", "Asks a question" | ink |
| `Finished` (not yet seen) | a small amber dot (the theme's accent) | the time | ink |
| `Idle` | the agent's mark, still and muted | the time | muted |
| `Failed(reason)` | the danger tone | "Stopped: <reason, cut to 40 characters>" | ink |

  `Finished` means "its changes are ready for you to look at". Opening the session clears it: the app sets
  `Idle`. A status change fades the new mark in (150 ms, `ease::OUT`); Reduce Motion skips the fade.

## The sidebar

`Sidebar` is an entity. `set_projects(projects, now)` gives it the data; it emits `SidebarEvent`
(`Open`, `NewSession`, `Retry`, `CloseProject`, `RevealProject`, `CopyPath`) with the ids the app gave.

- **One virtual list.** Every row is 32 px (`uniform_list`), so 50 projects and 2,000 sessions cost what a
  screenful costs. `sidebar_model::rows` decides the rows: a project header, its sessions, a "Show N older"
  fold, or "No sessions yet" and a New session action.
- **Order.** In a project, sessions that need you come first, then the most recent activity first; equal
  ones keep the app's order.
- **The fold.** A project shows five sessions, then "Show N older" (and "Show fewer"). A session that needs
  you or has news is never folded away: the fold moves past it.
- **Keys.** Up and down move, left folds a project or goes to it from one of its sessions, right unfolds or
  goes in, Home and End, Enter or Space open a session or fold a project, Esc closes the menu. The
  keyboard profile's next and previous file letters (`w`/`s`, or `j`/`k`) move too. `sidebar_model::step` decides,
  and a selection follows its row by id when the rows reorder.
- **The project header** shows a folder or a host mark and the host's name in a chip, the branch, a spinner
  and "Connecting…" or "Reconnecting…", or the danger mark, "Offline" and a Retry action; a `+` starts a
  session; `⋯` or a right-click opens Close project, Reveal in tree, Copy path.
- **Motion.** A new session enters with `Entrance`. A session whose row moved (one that now needs you goes
  to the top of its project) slides to its place with the Layout spring. Reduce Motion jumps.

## The panels

`AgentPanels` is an entity. `set_panels(panels, project_order)` gives it `PanelData { id, project, title,
look, status, content }`; `content` builds the panel's element and runs only while the panel is on screen.
It emits `PanelsEvent`: `Activated(id)`, `Closed(id)` (the app drops that panel from what it passes in), and
`StateChanged`. `state()` and `restore()` hold what the app remembers per window: the layout, the grouping
and each panel's width.

- **Side by side** (`panel_strip`). Columns of a set width (480, from 320 to 960; drag a column's right edge
  to resize) in a row that scrolls sideways: the trackpad, or shift and the wheel. After the wheel stops
  (140 ms) the scroll settles on the nearest column edge with the Layout spring. Only the columns in view
  and a 240 px margin are built and laid out (`panel_layout::Geometry::visible`); the rest keep their state
  in the entities the app made and take no layout. A new column slides in from the right with the Layout
  spring and fades in; closing one closes its gap the same way.
- **Grouping.** Grouped, the panels of a project sit together under a thin header (name, location mark,
  host chip) and the groups follow the sidebar's order of projects. Ungrouped, one row in open order.
- **Single view** (`panel_tabs`). One panel fills the area, and tabs at the top pick it. A tab shows the same
  status mark as a session row (the agent's mark, or the tone that replaces it), the title, and a `×`. Tabs
  scroll sideways, reorder by drag, and group by project under the project's name. The active tab is
  scrolled into view.
- **Keys** (GPUI actions, with a modifier, so they never type into a panel's input). GitQuiet's table has no
  command for these, so they are atelier's own, and the caps come from the same chord strings:

| Command | Chord |
| --- | --- |
| Next panel, previous panel | ⌘] and ⌘[ (⌃ off macOS) |
| Next tab, previous tab | ⌃Tab and ⌃⇧Tab, in the order the tabs are shown |
| Close tab | ⌘W |
| Side by side / single view | ⌘\ |
| Group by project | ⌘⇧G |

  Next and previous panel move the focus and scroll it into view in the strip; in the single view they
  move between tabs like ⌃Tab.

## Pure parts and their tests

Nothing here needs a window to test: `session_status` (words, marks, what is never folded away),
`sidebar_model` (sorting, the fold, the keys, the time since), `panel_layout` (column positions, the
visible range, where a scroll settles, revealing a column, grouping), `tab_order` (open, close, reorder,
cycle, grouping). 50 unit tests.

## Numbers

See `docs/performance.md`, "Sidebar and panels".

## What the app does

- Set the status from the session events: `Working` between a message and the turn's end, `NeedsYou` on a
  `Permission` event (or a question), `Finished` at `TurnEnded` when the session is not the one open,
  `Failed` on `Ended` with a failure, and `Idle` when the session is opened.
- Save `AgentPanels::state()` on `StateChanged`.
- Add the two stories to the gallery (see the report).
