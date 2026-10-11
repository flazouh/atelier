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

**A client** has a rail icon, a wide sidebar with its own list (chats, threads), and a pane. It may reuse the shared chat view or the shared mail view when its data fits, scoped to its own provider. Telegram, a plugin, uses the chat view with only Telegram chats. Mail uses the mail view. It also gives agent tools. Read tools are free. A write tool (send a message, send mail) asks each time and shows the exact text. An agent never sends mail alone.

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

## 5. Plugins

A **plugin** is how a client, a backend or a tool connector gets into Atelier without being part of the app. Telegram is a plugin. So could Linear or Slack be, later.

**What a plugin is:** a folder with a manifest and a program. Atelier starts the program as its own process and talks to it. The program is written in any language.

```
telegram/
  plugin.json     the manifest
  telegram        the program (or a command to run it)
  logo.svg
```

**The manifest** (`plugin.json`) says:
- `id`, `name`, `version`, `logo`.
- `kind`: `client` (a rail icon and a view), `backend` (a backend for a feature: tasks, workspace), or `tool` (agent tools only, no view).
- `implements`: the interface it fills: `messaging`, `tasks`, `mail`. A tool plugin lists its tools instead.
- `run`: the command, and the operating systems it runs on.
- `settings`: the fields it needs (for example a workspace name), as data. Atelier draws them in Settings.
- `permissions`: what it may do: which network hosts it reaches, whether it keeps a secret in the keychain. Atelier shows the list before the person installs it.

**The talk between Atelier and the plugin:** JSON-RPC 2.0 over the plugin's stdin and stdout. The calls are the calls of the interface the plugin implements, with the types of the schema we already have (`messaging.schema.json`, `tasks.schema.json`). Changes arrive as notifications (a new message). So the plugin only answers questions like "give me the history of this chat". It never draws.

**What Atelier does with it:** a `PluginProvider` wraps the process and implements the same trait as a built-in provider. It goes into the same registry. So the screen, the agent tools, the cards and the contract tests all work with no change. A `client` plugin also gets its rail icon, and the shared view scoped to it.

**Login in the plugin:** a plugin may ask for a login step as data: `{ "type": "qr", "data": "tg://login?token=..." }`, or `{ "type": "code", "prompt": "..." }`, or `{ "type": "fields", ... }`. Atelier draws the step (a QR code, an input) and sends the answer back. The plugin keeps its session through the host's keychain call, not in its own files.

**Install:** from the plugin list in Settings, Connectors (a registry of plugins kept in a public repository; anyone adds one by a pull request), or from a folder, a URL or a git address. The person sees the name, the author, the version and the permissions first.

**Safety:**
- A plugin runs with the person's rights and no more. It reaches only the hosts it listed.
- What a plugin returns is data: it is marked as data for the agent.
- Reads are free for the agent; every write asks.
- Every call is in the audit log. A kill switch stops one plugin or all.
- Its list of calls is pinned at install. A change shows to the person before it is used.
- Plugins are not code in the Atelier process, so a crash of one plugin is not a crash of the app.

**What a plugin cannot do in v1:** draw its own screen. A plugin that needs a view that none of our shared views fits waits for the view schema (a later step; the card schema is the first part of it).

## 6. The rail

Today the rail is a closed list in the shell: sessions, tasks, code, messages, mail. v1 makes the rail a list of **contributions** (the slot system exists; the rail was left out). A feature or a connector registers one rail view: id, icon, label, order, the sidebar, the pane, and a condition (a connector shows only when added). The built-in views register the same way.

Order: Sessions, Workspace, Tasks, Code, then the connectors in the order the person added them.

## 7. Telegram, the first plugin

- **What it is:** a full Telegram client you install and connect. Its rail icon opens the wide sidebar with your chats and the pane with the messages, written and read by you. It is a user client over MTProto, not a bot. The Rust options are `grammers-client` and `tdlib-rs`; a spike picks one.
- **Login:** a QR code. Telegram supports QR login: you scan the code in the phone app, and no phone number is typed. A phone number and code is the fallback. The session is kept in the keychain.
- **The agent:** once connected, the agent can read when you ask it to ("check my chat with Ana"), as it can for Slack and Gmail. Reads are free; writing asks each time and shows the exact text. There is no extra gate: it is your data and your choice to ask.
- **The key:** every third-party Telegram client uses its own `api_id` and `api_hash`, free from `my.telegram.org`, shipped in the client; it is not a secret. We register one for Atelier.
- **Rules of the terms we keep:** our own `api_id`, a name that says it is unofficial, no copying of their logos, show that the Telegram API is used, and no action the person did not ask for.
- **The one risk, said once:** Telegram's terms forbid the developer to use their data to train or develop AI. They are written for the developer who holds the `api_id`. If Telegram counts an agent that reads chats as AI use, they may revoke our key, and every user's Telegram client stops until we get a new one. For a personal tool the risk is small. For a product sold per seat it is larger, so a lawyer reads the clause before we sell it. It does not stop use now.
- **Install:** it is a plugin (section 5), `kind: client`, `implements: messaging`. It is not in the Atelier repo. It lives in its own repository and is listed in the plugin registry. The person installs it from Settings, Connectors, and its rail icon appears.
- **Login in the plugin:** the plugin asks for a `qr` login step; Atelier shows the QR code; the plugin keeps the session through the host's keychain call.

## 8. What changes in the code we have

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

## 9. Build order

1. Rail contributions, and Settings: Backends and Connectors (move Accounts under them).
2. The plugin host: the manifest, the JSON-RPC talk, `PluginProvider`, install and the permission screen, the login step, the audit log. Proved first with a tiny test plugin that fakes a chat service, then with Telegram.
3. Telegram as the first plugin (its own repository), with QR login.
4. Tool connectors (MCP): a URL or a command, sign-in, tools to every agent, cards. The first to try is Sentry.
5. The native workspace backend (people, then agents as members, then links and mentions).
6. Tool cards in chat for all of it (in progress).

## 10. Questions for you

1. **Name.** "Workspace" for the agentic space. Is that the word you want in the app?
2. **Plugin language.** Plugins may be written in any language and speak JSON-RPC over stdio. Is that right for you, or do you want Rust only?
3. **Several backends at once?** Today a project could show Linear and native together. Do you want only one active per feature, or a union?
4. **Native workspace and sessions.** Should every agent session also appear as a thread in a channel automatically, or only when someone sends it there?
5. **Telegram and the agent.** Connected means the agent can read when you ask, as for Slack and Gmail. Agree?
