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
| **Bot** | A named worker: face, role, skills, tools, memory, a harness and a provider. |
| **Template** | The master copy of a bot. A person or a workspace owns it. |
| **Copy** | A template added to a workspace. The workspace can read it, pin its version and set its rules. |
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
| `owner` | You, or a workspace. |
| `version` | Each edit makes a new version. A copy pins one. |

A bot is a **persona on top of a harness**. The harness runs the agent. The bot says who it is, what it knows and what it may touch. Two bots can share a harness. One bot cannot run on two harnesses at once.

### Whose bots

Your bots follow you. A workspace may not want every bot a person brings. So there are two owners and one rule.

- **Your template** is yours. You can use it in any project and in any workspace that allows it.
- **A workspace template** is owned by the workspace. Its admins edit it. Members use it.
- **Adding a bot to a workspace makes a copy.** The workspace sees exactly what the bot can do before it accepts: skills, tools, harness, provider. A later change to your template does not change the copy until the admin accepts the new version.
- **Policy per workspace:** `Off` (no personal bots), `Approved` (an admin accepts each bot), `Open` (any member may add one). The default is `Approved`.

This lets one person reuse a thorough debugging bot in every project. A company can still know every bot that touches its code.

## 4. Memory

Three layers. Each one stays where it was made.

1. **The bot's own memory.** What it learned about its job: "I always read the failing test first." It travels with your template.
2. **The workspace's memory.** Rules and facts of one workspace: "We squash merge." It stays in that workspace. A copy reads it. A template never takes it away.
3. **The project's memory.** Facts about one code base: "The gateway port is random." It stays with the project.

Say "Hey Benny, always remember this" and Benny writes to layer 1. Say "remember this for this project" and it writes to layer 3. The bot shows where it saved the note, so you can move it.

Company data does not leak. When you copy a template out of a workspace, layers 2 and 3 stay behind. Only layer 1 goes with it, and only what the bot wrote while it was your own template.

## 5. Playbooks

A playbook is an ordered list of bots and the hand-over between them. Deliver is the first one.

`Planner` → `Builder` → `Prover` → `Reviewer` → `Shipper`

- Each step hands over through the ticket, not through chat. The next bot reads the ticket and the proof so far.
- A step may stop and ask a person. That session shows in Everywhere as "needs you".
- A playbook belongs to a workspace or to you, like a bot.
- A person can start one step alone. Example: run only the Reviewer on a pull request.

## 6. The starter crew

The 62 skills we use today fall into a few roles. Each role becomes a starter bot. The first six have a face already.

| Bot | Role | Skills it loads |
|---|---|---|
| **Nimbus** (yellow) | Planner | `eng-plan`, `eng-grill`, `eng-issues`, `eng-handoff`, `eng-velocity` |
| **Bolt** (blue) | Builder | `eng-tdd`, `eng-architecture`, `eng-prototype`, `eng-parallel`, the `stack-*` skills for the project |
| **Pip** (green) | Prover | `eng-qa`, `eng-verify`, `control-browser`, `control-simulator`, `control-desktop`, `control-demo` |
| **Olive** (purple) | Reviewer | `eng-review`, `eng-quality-review`, `stack-knip`, `stack-pre-commit` |
| **Skip** (orange) | Shipper | `eng-deliver`, `eng-git`, `stack-railway`, `stack-secretctl`, `comms-slack` |
| **Dot** (pink) | Debugger | `eng-debug`, `eng-chrome-extension` |
| **Keyla** (red) | SSO Auditor | An example of a company bot: a read-only connector to the identity provider, checked each week. |

Four more roles need a face later: **Researcher** (`research*`), **Writer** (`content-*`, `comms-slack-voice`, `comms-gmail`, `comms-discord`), **Designer** (`design-*`) and **Operator** (`control-remote`, `eng-skills`).

Two skills are not bots. `content-adhd` and `content-ste100` are **house rules**: they set how every bot writes to one person. House rules belong to the person, not to a bot, and apply to all of the person's bots.

## 7. Where bots show

- **Sessions (the main view).** One row per session, with the bot's face and mood. You see at once which bot works, which waits for you and which is stuck.
- **Workspace chat.** Type `@` to pick a bot. The bot answers in a thread by default. The thread links its session.
- **The bot library.** A rail view to see, make and edit bots, and to add one to a workspace.
- **Status bar and sidebar.** The face at 18 to 30 px, with its mood.

## 8. What is not in v1

- Bots that hire other bots.
- A public store of bots. Sharing is by file or by workspace first.
- Voice or sound for bots.
- Billing per bot.

## 9. Questions for you

1. **Policy default.** Is `Approved` the right default for a new workspace?
2. **Tools on a copy.** A copy may use only the connectors the workspace already has. A bot that needs a new connector asks an admin. Agree?
3. **Where a bot answers.** Thread (my pick), channel, or side panel?
4. **Starter crew size.** Ship six bots at first, or all ten roles?
5. **Names.** Keep the names Bolt, Pip, Olive, Skip, Dot, Nimbus, Keyla, or rename?
