# Bots, v1 (draft for review)

Status: draft. Docs only. This says what a bot is, where it lives and who owns it. The faces and the motion are in `faces-v1.md`.

## 1. Why

Today a person loses track of what each agent session does. Threads sit in Slack, tabs and terminals. A **bot** gives each kind of work a name, a face, a set of skills and a memory. You see who works on what at a glance. You say "Hey Benny, always remember this" and Benny does.

## 2. The nouns

| Noun | What it is |
|---|---|
| **Workspace** | A place where people and bots work together. A person may belong to many. Example: Fluentai, Atelier. |
| **Project** | A code folder with its tasks and sessions. A project has zero or one workspace. |
| **Session** | One run of an agent on one piece of work. Every session belongs to a bot. |
| **Bot** | A named worker: face, role, skills, tools, memory, a harness and a provider. You own it. |
| **Playbook** | A named order of bots for one kind of job, such as Deliver. |

Rules:
1. A workspace has many projects. A project has zero or one workspace.
2. A new project needs no workspace. It works alone, with your own bots.
3. A person can belong to many workspaces. Sessions shows one workspace at a time, plus an Everywhere scope.
4. Everywhere shows only what needs you: approvals, questions, failures. It does not show work in progress.

## 3. Bots

A bot has these fields.

| Field | Meaning |
|---|---|
| `name`, `role` | "Dot", "Debugger". |
| `face` | A body, a colour and a tool, from the face data (`faces-v1.md`). |
| `harness` | The agent program: Claude Code, Codex, Cursor. |
| `provider` | The model service and model name. |
| `skills` | The skills it loads. |
| `tools` | The connectors it may use, each marked read or write. |
| `voice` | How it talks: calm, cheerful, short, thorough. |
| `memory` | What it learned, in three layers (section 4). |
| `owner` | You. |
| `version` | Each edit makes a new version. |

A bot is a **persona on top of a harness**. The harness runs the agent. The bot says who it is, what it knows and what it may touch. Two bots can share a harness. One bot cannot run on two harnesses at once.

### Whose bots

**v1 has no sharing rules.** You use your own bots, alone. There are no workspace policies and no copies of a bot. A workspace shows the bots of the people in it, and nothing else.

Later, when teams need it, a bot can become a template that a workspace copies, pins and approves. The shape is in section 8. It is not built now.

## 4. Memory

Three layers. Each one stays where it was made.

1. **The bot's own memory.** What it learned about its job: "I always read the failing test first." It follows the bot.
2. **The workspace's memory.** Rules and facts of one workspace: "We squash merge." It stays in that workspace. Every bot there can read it.
3. **The project's memory.** Facts about one code base: "The gateway port is random." It stays with the project.

Say "Hey Benny, always remember this" and Benny writes to layer 1. Say "remember this for this project" and it writes to layer 3. The bot shows where it saved the note, so you can move it.

Company data does not leak. Layers 2 and 3 stay with the workspace and the project. Only layer 1 follows the bot.

## 5. Playbooks

A playbook is an ordered list of bots and the hand-over between them. Deliver is the first one.

`Planner` → `Builder` → `Prover` → `Reviewer` → `Shipper`

- Each step hands over through the ticket, not through chat. The next bot reads the ticket and the proof so far.
- A step may stop and ask a person. That session shows in Everywhere as "needs you".
- A playbook belongs to a workspace or to you, like a bot.
- A person can start one step alone. Example: run only the Reviewer on a pull request.

## 6. The starter crew

All ten roles ship. The 62 skills we use today fall into these roles, one bot each.

| Bot | Colour | Role | Skills it loads |
|---|---|---|---|
| **Nimbus** | yellow | Planner | `eng-plan`, `eng-grill`, `eng-issues`, `eng-handoff`, `eng-velocity` |
| **Bolt** | blue | Builder | `eng-tdd`, `eng-architecture`, `eng-prototype`, `eng-parallel`, the `stack-*` skills for the project |
| **Pip** | green | Prover | `eng-qa`, `eng-verify`, `control-browser`, `control-simulator`, `control-desktop`, `control-demo` |
| **Olive** | purple | Reviewer | `eng-review`, `eng-quality-review`, `stack-knip`, `stack-pre-commit` |
| **Skip** | orange | Shipper | `eng-deliver`, `eng-git`, `stack-railway`, `stack-secretctl`, `comms-slack` |
| **Dot** | pink | Debugger | `eng-debug`, `eng-chrome-extension` |
| **Quill** | blue | Researcher | `research`, `research-current-art`, `research-literature`, `research-user-pain` |
| **Ink** | purple | Writer | `content-*`, `comms-slack-voice`, `comms-gmail`, `comms-discord` |
| **Mimi** | pink | Designer | `design-*` |
| **Gus** | green | Operator | `control-remote`, `eng-skills` |

Seven brand colours cover ten bots, so some repeat. The body shape and the tool tell two bots of one colour apart.

The first six have faces. Quill, Ink, Mimi and Gus need new faces first (`faces-v1.md`).

**Keyla** (red, SSO Auditor) is an example of a company bot, not a starter bot. She is read-only on the identity provider and checks it each week.

Two skills are not bots. `content-adhd` and `content-ste100` are **house rules**: they set how every bot writes to one person. House rules belong to the person, not to a bot, and apply to all of the person's bots.

## 7. Where bots show

- **Sessions (the main view).** One row per session, with the bot's face and mood. You see at once which bot works, which waits for you and which is stuck.
- **Workspace chat.** Type `@` to pick a bot. The bot answers in a thread under your message. The channel stays quiet. The thread links its session.
- **The bot library.** A rail view to see, make and edit bots, and to add one to a workspace.
- **Status bar and sidebar.** The face at 18 to 30 px, with its mood.

## 8. What is not in v1

- Sharing bots between people: templates, copies pinned by a workspace, and a policy per workspace (`Off`, `Approved`, `Open`). Copies would use only the connectors the workspace already has.
- Bots that hire other bots.
- A public store of bots. Sharing is by file or by workspace first.
- Voice or sound for bots.
- Billing per bot.

## 9. Decisions

Answered by Alex:

1. **Policy.** No workspace policy in v1. You use your own bots.
2. **Tools on a copy.** Not needed. There are no copies in v1.
3. **Where a bot answers.** In a thread under the message.
4. **Crew size.** All ten roles ship.
5. **Names.** Keep Nimbus, Bolt, Pip, Olive, Skip, Dot and Keyla. Add Quill, Ink, Mimi and Gus.

No open questions remain in this file.

## 10. In code

The crate `atelier-bots` (`crates/bots`) holds this model. It has no screen and no dependency on the UI.

- `Bot`, `FaceChoice`, `Provider`, `ToolGrant`, `Playbook`, `PlaybookStep` and `MemoryNote` are the data. `BotId` is a short name of lower case letters, digits and hyphens, and it is the name of the file.
- `BotStore` is the trait the app talks to. `DiskBotStore` keeps one JSON file for each bot and each playbook, and one file of notes for each layer:

```
<folder>/bots/<id>.json
<folder>/playbooks/<id>.json
<folder>/memory/bot/<id>.json
<folder>/memory/workspace/<id>.json
<folder>/memory/project/<path, made safe>.json
```

- An edit that changes a bot makes the next `version`. A save that changes nothing keeps the version.
- A playbook can only name bots that are kept, and a bot that a playbook names cannot be removed.
- `starter_crew()` is the ten bots and `starter_playbooks()` is Deliver. `seed_starters` adds the ones a folder does not hold. It never touches a bot that is kept, even after you edit it.
- The faces of Quill, Ink, Mimi and Gus are provisional until the faces step.

A session may belong to a bot (`crates/app/src/session_bot`). A session started the old way has no bot.

- The bot's profile has a "Start a session" button. The control socket takes `new_session` with a `bot` id.
- The session runs on the bot's harness. The settings keep the bot's id with the session, so the next launch still knows it.
- `Bot::persona` is the bot's name, role, job, voice and skills as text. Claude Code gets it added to its system prompt at each launch. The other harnesses have no such option, so they get nothing.
- The session's row in the sidebar and its header show the bot's face. `mood_of` gives the mood from what the session does (`faces-v1.md`, section 3). The face on the row is 20 px and moves while the session works.
- The tools a bot may use are not enforced yet.

