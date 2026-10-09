# Features, backends and connectors, v1 (draft for review)

Status: draft. Docs only. This says what Atelier is made of, so the next pieces of code have a clear shape.

## 1. The picture

There are three kinds of thing. Only the first two have a view.

| | Feature | Client | Tool connector |
|---|---|---|---|
| What it is | Something Atelier owns: tasks, the agentic workspace, git | A door to a service you read and write yourself: Telegram, mail | A service the agent uses, like a connector in Claude Code: Sentry, PostHog, any MCP server |
| Native version | Yes, always | No | No |
| Backends | Native, or an outside service behind the same screen | One: the service itself | One: the service itself |
| Rail icon and view | Yes, one per feature | Yes, one per client you add | **None** |
| Agents | Tools for the feature | Tools for the client | The tools of the connector |
| Added by | Built in | Settings, Connectors | Settings, Connectors (a URL or a command, and a sign-in) |

Three rules hold for all of them:
1. **Atelier draws the screen.** A backend or a client gives data and takes actions. It never draws.
2. **Agents can reach everything that is connected.** Features, clients and tool connectors reach every agent through the one gateway. A person sets a connector up once, not once per agent.
3. **Reads are free, writes ask.** A write tool shows the exact action and waits for the person. Text that comes back from a service is data, never instructions.

## 2. Features and their backends

A feature has one native backend that ships with Atelier and works with no account. A person may point a feature at another backend.

- **Tasks**: native (today's local tracker), Linear, GitHub Issues.
- **Workspace** (the agentic workspace, section 4): native, Slack, Discord.
- **Git and reviews**: native git plus GitHub (exists today; not in this spec).

The choice is **per project**: "the tasks of this project live in Linear." One backend is active for a feature in a project. The sidebar and the pane do not change when the backend changes. A control that a backend cannot support is hidden (the capability list does this today).

Moving between backends uses export and import (in the tasks spec): a team that leaves Slack for the native workspace takes its history.

## 3. Clients and tool connectors

**A client** has a rail icon, a wide sidebar with its own list (chats, threads), and a pane. It may reuse the shared chat view or the shared mail view when its data fits, scoped to its own provider. Telegram uses the chat view with only Telegram chats. Mail uses the mail view. It also gives agent tools. Read tools are free. A write tool (send a message, send mail) asks each time and shows the exact text. An agent never sends mail alone.

**A tool connector** has no view. It is an MCP server: a URL or a command to start, and a sign-in when it needs one (OAuth, run by Atelier; the token lives in the keychain). Once added:
- Its tools appear for every agent through the gateway, with the connector's name in front (`sentry_search_issues`).
- A tool that only reads is free. Every other tool asks, unless the person marks it always allowed.
- Chat shows each call as a card with the connector's logo. A connector may ship a card schema for its tools (the card schema spec); without one the call shows the generic card.
- What a tool returns is untrusted text. The gateway marks it as data, as it does for chat and mail.
- Every call is written to an audit log: who (which agent, for which person), which tool, when.
- A connector's list of tools is pinned when it is added. If the tools change later, the person sees the change before the new tools are offered.
- A kill switch per connector and for all of them.

A person adds a client or a connector from Settings, Connectors: "Add Telegram", "Add a connector". A client's icon appears on the rail. Removing either removes its tools for every agent.

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

## 6. Telegram, the first client

- **Client:** a user client over MTProto, not a bot. The Rust options are `grammers-client` and `tdlib-rs`. We pick one in a spike, in a plugin-sized crate, behind the chat interface.
- **Rules from Telegram's terms:** our own `api_id`, the name "Unofficial" in the client's identity, no copying of their logos, show that the Telegram API is used. We do nothing the person did not ask for.
- **A hard line:** Telegram's terms forbid using their data to train or develop AI. So the **agent read tool for Telegram is off by default** and the person switches it on knowing this. A lawyer reads the clause before we ship it on. The view itself (a person reading and writing) is allowed.
- **Login:** phone number and code, in Atelier; the session is kept in the keychain.

## 7. What changes in the code we have

| Today | v1 |
|---|---|
| Tasks screen shows every provider side by side | Per project: one backend, chosen in Settings, Backends |
| Messages screen shows every chat provider | The Workspace view shows the backend of the project; a connector gets its own rail view with the same screen |
| Mail has the generic shape of a feature | Mail is a client with its own rail icon |
| Settings, Accounts lists everything | Settings, Backends (per feature) and Settings, Connectors |
| Rail is a closed list | Rail is contributions |
| No native chat | The native workspace, section 4 |
| No Telegram | The client, section 6 |
| No way to add a tool-only service | Tool connectors (MCP), section 3 |

The providers, the gateway, the agent tools, the contract suites and the screens we have are kept.

## 8. Build order

1. Rail contributions, and Settings: Backends and Connectors (move Accounts under them).
2. The native workspace backend (people, then agents as members, then links and mentions).
3. Tool connectors (MCP): add by URL or command, sign-in, the tools reach every agent, cards in chat. The first one to try is Sentry.
4. Telegram client (spike, then the view).
5. Tool cards in chat for all of it (in progress).

## 9. Questions for you

1. **Name.** "Workspace" for the agentic space. Is that the word you want in the app?
2. **Several backends at once?** Today a project could show Linear and native together. Do you want only one active per feature, or a union?
3. **Native workspace and sessions.** Should every agent session also appear as a thread in a channel automatically, or only when someone sends it there?
4. **Telegram agent tool off by default.** Agree?
