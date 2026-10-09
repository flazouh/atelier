# The `tasks` capability, v1 (draft for review)

Status: draft. No code changes come with this document. Read it, change it, then it becomes the contract.

## 1. Why this document

Atelier will have one screen for tasks. Linear, GitHub Issues, Atelier's own tasks and others are **providers**: they supply data
and take actions, and the screen does not change. Agents talk to the same small vocabulary whatever the provider. A team that
moves from Linear to Atelier changes the provider behind the screen and imports its history.

Words used here:
- **Capability**: an interface and its one screen (`tasks`, later `git`, `messaging`, `mail`).
- **Provider**: a plugin that implements a capability.
- **Contribution**: a plugin piece that fills a slot (not part of this document).

## 2. What exists today (crates/tracker, about 2000 lines)

- A blocking trait `Tracker` with `list`, `get`, `create`, `create_many`, `update`, `record`, `activity`, `tasks_of_session`,
  `tasks_of_pr`, `labels`, `subscribe`. Two implementations: `LocalTracker` (SQLite file per project) and `RemoteTracker`
  (the same calls over SSH). The app uses only `Arc<dyn Tracker>`, in about 10 files.
- A `Task` with six statuses, priority 0 to 4 (Linear's numbering), an assignee that is a person or an agent, labels as strings,
  a project as a string, a parent, links to agent sessions and pull requests.
- Not there: user, project and label as entities; comments as objects; custom workflow states; paging; export and import;
  external ids; error cases for offline, signed out and rate limits; a list of what a provider can do; async calls;
  actor identity (the `by` argument is free text); `tasks_of_pr` takes a number without the repo.

This v1 keeps what works and adds what a second provider needs. Nothing here removes a call the app uses.

## 3. Rules for every capability

1. **Every thing has a reference.** `ref = <capability>:<provider>:<account>:<id>`, for example `tasks:linear:acme:ENG-123` or
   `tasks:local:atelier:LAT-42`. A reference is stable and is the same text everywhere: in a task, in a message, in a pull
   request, in an agent session. This is how things link across capabilities.
2. **Keep the original.** Each entity has `raw`: the provider's own JSON. The neutral fields are a view of it. Export and import use
   `raw`, so a move loses nothing.
3. **Say what you can do.** A provider returns a `capabilities` value (section 6). An operation or a field it does not support is
   absent from the list. The screen and the agent tools show only what the list allows.
4. **People and agents are the same kind of actor** (section 4.3). Every change records who made it and for whom.
5. **Time is milliseconds since the epoch, UTC.** (Today's tracker uses seconds: the adapter multiplies.)
6. **Calls never run on the UI thread, and lists are paged.** The Rust interface blocks, as the rest of the app does, and the app calls it from a background thread. A list returns a page and a cursor. A binding for another language (Swift) may use async calls with the same shape.

## 4. Entities

The machine-readable form is `tasks.schema.json` next to this file. This is the same content in words.

### 4.1 Task
`ref`, `key` (the human id: `ENG-123`, `#42`), `title`, `description` (markdown), `status` (4.2), `priority`, `assignees` (a list of
actors; most providers use one), `labels` (a list of label refs), `project` (a project ref or none), `parent` (a task ref or
none), `links` (4.5), `created_at`, `updated_at`, `version` (for safe updates), `raw`.
Optional fields, present only when the feature is on: `due_at`, `estimate`, `relations`, `cycle`, `attachments`.

### 4.2 Status
`{ id, name, category }`. The **category** is one of `backlog, todo, in_progress, in_review, done, canceled`: this is what the
screen groups by and what agents reason about. The **name** is the team's own word ("Ready for QA"). A provider without custom
states uses the six names. Mapping: Linear's state types `backlog, unstarted, started, completed, canceled` map to
`backlog, todo, in_progress, done, canceled`, and a started state whose name says "review" maps to `in_review`. GitHub Issues
maps `open` to `todo` or `in_progress` (by a label) and `closed` to `done` or `canceled` (by `state_reason`).

### 4.3 Actor
`{ kind: person | agent, id, name, on_behalf_of? }`. An agent that works for a person carries that person's id in `on_behalf_of`.
Rights never exceed the person's. A message or comment made by an agent shows as "Alex's agent".

### 4.4 Priority
`none, urgent, high, medium, low` in the interface, numbered 0 to 4 as today's tracker numbers them. The adapter keeps the number.

### 4.5 Link
`{ kind: session | pull_request | task | message | mail | url, ref }`. This is the cross-capability link. A `pull_request` link
carries the full reference including the repository (this fixes `tasks_of_pr(number)`, which cannot tell repositories apart).

### 4.6 Comment, Activity, Label, Project
- **Comment** `{ ref, task, author: actor, body, created_at, updated_at, raw }`: a real entity, not a row of the activity log.
- **Activity** `{ ref, task, at, by: actor, kind, detail }` with the kinds the tracker has (created, status changed, assigned,
  edited, commented, session started, PR opened, PR merged, commit).
- **Label** `{ ref, name, color? }` and **Project** `{ ref, key, name }`: entities now, strings before.

## 5. Operations

Core (every provider):

| Operation | Takes | Gives |
|---|---|---|
| `capabilities()` | none | the list in section 6 |
| `whoami()` | none | the actor the credentials belong to |
| `list(query, cursor)` | filters, sort, page size | a page of tasks and the next cursor |
| `get(ref)` | a task reference | a task |
| `create(new, actor)` | title, description, status, priority, project, labels, assignees, parent | the task |
| `update(ref, patch, version, actor)` | the fields to change (a field set to `null` clears it) | the task, or a `Conflict` if `version` is old |
| `comment(ref, body, actor)` | text | the comment |
| `labels()` | none | the labels in use (an empty list when the provider has none) |
| `projects()` | none | the projects in use (an empty list when the provider has none) |
| `activity(ref, cursor)` | a task | a page of activity |
| `subscribe(filter)` | what to follow | a stream of `created`, `updated`, `activity` events |

Optional (a provider lists the ones it has): `create_many`, `delete`, `link`/`unlink`, `statuses()`,
`relations`, `cycles`, `attachments`, `webhooks` (push) as against polling, `export`, `import`.

`export(cursor)` gives every entity as `{ entity, raw }`, in order, with a cursor. `import(batch, id_map)` takes the same shape,
writes in one transaction, and is **idempotent**: a second run with the same batch changes nothing. This is the migration path.

## 6. Capabilities value

`{ operations: [...], features: [custom_states, subtasks, relations, estimates, due_dates, cycles, attachments, webhooks,
reactions], limits: { page_max, rate: { per_minute? } }, auth: [oauth, token, browser_session, none] }`.
The screen reads this and hides what is missing. An agent tool that the provider lacks is not offered to the agent at all.

## 7. Errors

`NotFound`, `Invalid { field }`, `Conflict { current }`, `Offline`, `NotSignedIn`, `RateLimited { retry_after }`,
`Unsupported { feature }`, `Storage`, `Provider { code, message }`. Today's `Unsupported("keep tasks")` becomes
`Unsupported { feature }` and is no longer a string to parse.

## 8. The agent tools (what the MCP gateway shows)

`tasks.list`, `tasks.get`, `tasks.create`, `tasks.update`, `tasks.comment`, `tasks.search`. Each takes an `account` (which
provider and workspace). Permission classes: `list`, `get`, `search`: read, no prompt. `create`, `comment`: write, prompt once per
chat. `update` of status or assignee: write. `delete`: always ask. Anything an agent reads from a task body is marked untrusted.

### 8.1 Tool cards

Every tool call of an agent shows as a card in the chat. A tool has one of three kinds of card.

1. **Capability tool.** `tasks.search`, `mail.get` and the like. Atelier draws one card per tool, once, from the neutral result. It works for every provider (Linear and GitHub for tasks, Gmail and IMAP for mail).
2. **Plugin tool with a card schema.** A plugin that has no capability (Sentry, PostHog, Calendar) ships a **card schema** for each of its tools. The schema is JSON data, not code: a title, fields, badges (for example level and count), actions, and a link to an entity by `ref`. The host draws it. The plugin runs no code in the UI.
3. **Fallback.** A tool with no card schema gets the generic card: name, arguments, and the result as folded JSON.

All cards share one header: the provider's **logo and name** from its manifest, the account, and a short line such as "Searched Linear, 3 tasks". All cards have four states: running, done, failed, and waiting for approval (write tools). An agent's card shows the origin: "Alex's agent". An entity in a card opens in its shared screen, by `ref`.

A provider of a capability implements the **tool interface** and returns the neutral result. It draws nothing.

**Promotion.** When a second provider appears for a plugin's domain (for example Datadog next to Sentry), the domain becomes a capability. The card schema becomes the one shared card, and the first plugin moves to it.

## 9. Contract tests

Every provider must pass one shared suite, including the local one:
1. `create` then `get` returns what was sent.
2. `update` changes only the fields in the patch, and `null` clears a field.
3. An `update` with an old `version` returns `Conflict`.
4. A list is stable across pages: no task twice, none missed, while the data does not change.
5. `capabilities` is honest: each operation not listed returns `Unsupported`, each listed one works.
6. `export` then `import` into an empty provider gives the same neutral view, and a second `import` changes nothing.
7. `subscribe` delivers each change once, in order.
8. The `by` actor is kept, with `on_behalf_of`.
9. A reference parses back to its provider and id.
10. Each tool result fits the card of its tool: a result that does not fit fails the test.

## 10. How the current code gets there (additive)

1. Add the types of this document beside the current ones, with adapters both ways. No app call changes.
2. Put provider dispatch behind `Project::tracker()`: choose the provider by configuration, with its credentials from the
   keychain (`atelier_settings::secrets`, one named key per provider and account).
3. Add `capabilities()`, paging and `Offline`/`NotSignedIn`/`RateLimited`. The app handles them in the one tasks screen.
4. Build the Linear provider as the second implementation. It passes the contract suite or the spec changes.
5. Only then freeze v1 and publish the schema for the iOS app and for plugin authors.

## 11. Open questions for you

1. **`in_review` as a category.** Linear has no such type. I map by the state's name. Keep six categories, or five and let
   `in_review` be a name?
2. **One assignee or many.** GitHub allows many, Linear one. I allow a list. Keep it?
3. **Where do comments live when a team moves?** `import` writes them. Do we also keep the original author as a person who may
   not exist in Atelier yet (a "ghost" actor)?
4. **Local provider as the reference.** `LocalTracker` becomes "Atelier tasks". For the team hub later, it gets a server. Do we
   keep the SQLite one as the offline provider for a single user?
5. **Sync.** Polling is the start (as the remote tracker does). Which providers get push first (Linear webhooks)?
