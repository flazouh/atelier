# Features, backends and connectors, v1 (draft for review)

Status: draft. Docs only. This says what Atelier is made of, so the next pieces of code have a clear shape.

## 1. The picture

The narrow rail on the left has an icon for each **view**. A view opens a wide sidebar (the list: discussions, tasks, mail threads) and a main pane. There are two kinds of view.

| | Feature | Connector |
|---|---|---|
| What it is | Something Atelier owns: tasks, the agentic workspace, git | A door to something outside: Telegram, mail |
| Native version | Yes, always | No |
| Backends | Native, or an outside service behind the same screen | One: the service itself |
| Example | Tasks: native, or backed by Linear. Workspace: native, or backed by Slack or Discord | Telegram. Gmail |
| Rail icon | One per feature | One per connector you add |
| Agents | Tools for the feature | Tools for the connector |

Two rules hold for both kinds:
1. **Atelier draws the screen.** A backend or a connector gives data and takes actions. It never draws.
2. **Agents can reach everything that is connected.** Each feature and each connector gives agent tools (list, read, search, and with approval write), through the one gateway we already have.

## 2. Features and their backends

A feature has one native backend that ships with Atelier and works with no account. A person may point a feature at another backend.

- **Tasks**: native (today's local tracker), Linear, GitHub Issues.
- **Workspace** (the agentic workspace, section 4): native, Slack, Discord.
- **Git and reviews**: native git plus GitHub (exists today; not in this spec).

The choice is **per project**: "the tasks of this project live in Linear." One backend is active for a feature in a project. The sidebar and the pane do not change when the backend changes. A control that a backend cannot support is hidden (the capability list does this today).

Moving between backends uses export and import (in the tasks spec): a team that leaves Slack for the native workspace takes its history.

## 3. Connectors

A connector is a client. It has a rail icon, a wide sidebar with its own list (chats, threads), and a pane. It may reuse the shared chat view or the shared mail view when its data fits, scoped to its own provider. Telegram uses the chat view with only Telegram chats. Mail uses the mail view.

For agents, a connector gives tools. Read tools are free. A write tool (send a message, send mail) asks the person each time, shows the exact text, and an agent never sends mail alone.

A connector is added from Settings, Connectors: "Add Telegram". The icon then appears on the rail. Removing the connector removes the icon and the tools.

## 4. The native workspace

The workspace is where people and agents work together. It is the native backend of the Workspace feature, and the part that can replace Slack for a team.

What it holds:
- **Channels** (open, private) and **direct chats**.
- **Members**: people and agents. An agent is a member like a person, with its own name and mark, and it acts for a person ("Alex's agent").
- **Messages and threads**, with files and reactions.
- **Links**: a message can carry a task, a session, a pull request or a file, by reference (`tasks:...`, `session:...`). A task shows the discussion that made it.
- **Agent presence**: an agent in a channel can read it (with the person's permission) and answer when mentioned. A mention starts or continues an agent session. The session's result posts back as a message.

Storage: one SQLite database per project, as tasks have today. Later a team hub (a server) makes it shared. The interface is the messaging interface we already have, so Slack and Discord and the native workspace are interchangeable backends.

Not in v1: calls, presence dots, huddles, a search index beyond text.

## 5. The rail

Today the rail is a closed list in the shell: sessions, tasks, code, messages, mail. v1 makes the rail a list of **contributions** (the slot system exists; the rail was left out). A feature or a connector registers one rail view: id, icon, label, order, the sidebar, the pane, and a condition (a connector shows only when added). The built-in views register the same way.

Order: Sessions, Workspace, Tasks, Code, then the connectors in the order the person added them.

## 6. Telegram, the first connector

- **Client:** a user client over MTProto, not a bot. The Rust options are `grammers-client` and `tdlib-rs`. We pick one in a spike, in a plugin-sized crate, behind the chat interface.
- **Rules from Telegram's terms:** our own `api_id`, the name "Unofficial" in the client's identity, no copying of their logos, show that the Telegram API is used. We do nothing the person did not ask for.
- **A hard line:** Telegram's terms forbid using their data to train or develop AI. So the **agent read tool for Telegram is off by default** and the person switches it on knowing this. A lawyer reads the clause before we ship it on. The view itself (a person reading and writing) is allowed.
- **Login:** phone number and code, in Atelier; the session is kept in the keychain.

## 7. What changes in the code we have

| Today | v1 |
|---|---|
| Tasks screen shows every provider side by side | Per project: one backend, chosen in Settings, Backends |
| Messages screen shows every chat provider | The Workspace view shows the backend of the project; a connector gets its own rail view with the same screen |
| Mail has the generic shape of a feature | Mail is a connector with its own rail icon |
| Settings, Accounts lists everything | Settings, Backends (per feature) and Settings, Connectors |
| Rail is a closed list | Rail is contributions |
| No native chat | The native workspace, section 4 |
| No Telegram | The connector, section 6 |

The providers, the gateway, the agent tools, the contract suites and the screens we have are kept.

## 8. Build order

1. Rail contributions, and Settings: Backends and Connectors (move Accounts under them).
2. The native workspace backend (people, then agents as members, then links and mentions).
3. Telegram connector (spike, then the view).
4. Tool cards in chat for all of it (in progress).

## 9. Questions for you

1. **Name.** "Workspace" for the agentic space. Is that the word you want in the app?
2. **Several backends at once?** Today a project could show Linear and native together. Do you want only one active per feature, or a union?
3. **Native workspace and sessions.** Should every agent session also appear as a thread in a channel automatically, or only when someone sends it there?
4. **Telegram agent tool off by default.** Agree?
