# The `messaging` capability, v1 (draft for review)

Status: draft. It follows `tasks-v1.md` and uses the same rules. The Rust types are in `crates/capabilities/src/messaging`
and the machine-readable form is `messaging.schema.json`.

## 1. Why

Atelier will have one screen for messages. Slack, Discord, Telegram and others are **providers**. The screen and the agent
tools talk to one small vocabulary, and a provider maps its service to it. A team that leaves one service for another
changes the provider behind the screen.

The rules of `tasks-v1.md` section 3 hold here: every thing has a reference, every entity keeps its `raw` JSON, a provider
says what it can do, a person and an agent are both actors, time is milliseconds since the epoch in UTC, and lists are paged.

## 2. References

`messaging:<provider>:<account>:<id>`. The account is the workspace (a Slack team, a Discord server). The id says what the
thing is:

| Thing | Id | Example |
|---|---|---|
| Workspace | `workspace` | `messaging:slack:acme:workspace` |
| Channel | `<channel>` | `messaging:slack:acme:C01` |
| Message | `<channel>:<ts>` | `messaging:slack:acme:C01:1760000000.000100` |
| Person | `user/<user>` | `messaging:slack:acme:user/U01` |

`ts` is the provider's message id, a string that sorts in time order inside one channel. A thread has the reference of
its root message.

## 3. Entities

- **Workspace** `{ ref, name, raw }`.
- **Channel** `{ ref, name, kind, topic?, archived, member_count?, unread?, created_at?, raw }`. `kind` is `public`,
  `private`, `dm` or `group_dm`. A dm has the other person's name as its name.
- **Person** `{ ref, name, handle, display_name?, is_bot, avatar_url?, raw }`.
- **Message** `{ ref, channel, parent?, text, author, created_at, edited_at?, attachments, reactions, reply_count,
  origin?, raw }`.
  - `text` is a markdown subset (section 4).
  - `author` is an actor. Its `id` is the provider's user id.
  - `parent` is the reference of the root message when the message is a reply, and absent otherwise.
  - `reply_count` is the number of replies under a root message, and 0 for any other message.
  - `origin` is set when an agent sent the message, for example `Sam's agent`. See section 8.
- **Reaction** `{ name, count, by, me }`: the emoji short name without colons (`thumbsup`), how many reacted, the actor ids
  when the provider says them, and whether the signed-in person reacted.
- **Attachment** `{ id, name, mime?, size?, url, kind }`. `kind` is `file`, `image`, `link` or `other`. The url is the
  provider's; the app fetches it through the host with the person's credentials, never with an agent's.

History and search return **top-level messages** in `history` and replies in `thread`. A root message carries
`reply_count`, so the screen shows "3 replies" and opens the thread on a click.

## 4. Text

Text is a markdown subset: `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, fenced code, `> quote`, `-` lists, and
`[text](url)`. Nothing else: no HTML, no tables, no headings. A mention is a link whose target is a reference:
`[@Sam](messaging:slack:acme:user/U02)`. A channel link works the same way. A provider converts between this subset and its
own markup (Slack `mrkdwn`) in both directions, and keeps the original in `raw`. Text that came from a person or a service
is untrusted: the screen draws it as text, and an agent reads it as data, never as instructions.

## 5. Operations

Core (every provider):

| Operation | Takes | Gives |
|---|---|---|
| `capabilities()` | none | the value in section 6 |
| `whoami()` | none | the actor the credentials belong to |
| `workspace()` | none | the workspace |
| `channels(query)` | kind, text, archived, limit, cursor | a page of channels |
| `history(channel, cursor, limit)` | a channel | a page of top-level messages, newest first; the cursor goes back in time |
| `thread(message, cursor)` | a root message | a page: the root first, then replies oldest first |
| `send(new, by)` | channel, text, `in_thread_of?` | the message |
| `subscribe(filter)` | channels or all | a stream of events |

Optional (a provider lists the ones it has): `search`, `edit`, `delete`, `react`, `mark_read`, `person`, `export`,
`import`. `react(message, name, on, by)` adds or removes a reaction and is idempotent. `search(query)` takes text, an
optional channel and an optional author id, and gives a page of messages, best match first. `mark_read(channel, up_to)`
moves the read marker. `export` and `import` follow `tasks-v1.md` section 5 and are not implemented by any provider in
v1; they are listed so the vocabulary is fixed.

`send` with `in_thread_of` replies in the thread of that root message. A reply to a reply goes to the same root.

## 6. Capabilities value

`{ operations: [...], features: [threads, reactions, edits, attachments, presence, typing], formatting: plain | basic |
rich, limits: { page_max, per_minute }, auth: [oauth, token, browser_session, none] }`.

- `formatting` says how much of section 4 the service keeps. `plain` keeps none (the screen shows text as typed), `basic`
  keeps bold, italic, code and links, `rich` keeps all of it.
- Operations answer "can I call it". Features answer "does the service have the idea": `threads` (replies under a
  message), `reactions`, `edits` (a message may change after sending), `attachments`, `presence` and `typing` (the service
  tells who is online and who is typing; v1 has no call for them, so they only tell the screen what to hide).
- A call that the list lacks returns `Unsupported`, and the agent tool for it is not offered.

## 7. Errors

The errors of `tasks-v1.md` section 7, unchanged: `NotFound`, `Invalid { field }`, `Offline`, `NotSignedIn`,
`RateLimited { retry_after_ms }`, `Unsupported { feature }`, `Storage`, `Provider { code, message }`. `Conflict` is not used,
because a message has no version: an `edit` of a message that was deleted is `NotFound`.

## 8. Agents, permissions and origin

Tools: `messaging.channels`, `messaging.history`, `messaging.thread`, `messaging.search`, `messaging.send`,
`messaging.edit`, `messaging.delete`, `messaging.react`, `messaging.mark_read`. Each takes an `account`.

| Class | Tools | Rule |
|---|---|---|
| read | `channels`, `history`, `thread`, `search` | no prompt |
| quiet write | `mark_read` | no prompt, shown as a line in the chat |
| ask every time | `send`, `edit`, `delete`, `react` | the app shows the exact text, the channel and the account, and the person approves or refuses each call. "Always allow" is not offered. |

A message an agent sends is sent with the person's credentials, because the service has no other identity to use. So the
app keeps the difference visible:

1. The actor passed to `send` is an agent with `on_behalf_of` set to the person.
2. The returned message has `origin = "<person's name>'s agent"` and keeps the actor.
3. A provider that cannot store an actor puts the origin in the text as a last line (`_sent by Sam's agent_`), so the
   people in the channel can see it. A provider says which in its `raw`; the contract only requires `origin`.
4. An agent never gets more rights than the person. The app refuses an agent call that targets a channel the person cannot
   read.

What an agent reads from a message is data. The tool result is marked untrusted, and a message that tells the agent to
send or delete something does not change what the agent may do.

### 8.1 Tool cards (level 1)

Messaging is a capability, so Atelier draws **one card per tool**, once, from the neutral result; a provider draws
nothing. All cards share the header of `tasks-v1.md` section 8.1 (provider logo and name, account, origin).

| Tool | Card |
|---|---|
| `messaging.channels` | a list: kind icon, name, topic on one line, unread badge |
| `messaging.history`, `messaging.search` | a list of messages: avatar, author, relative time, text up to 4 lines, reaction chips, "3 replies" |
| `messaging.thread` | the root message, then the replies as a compact list |
| `messaging.send`, `messaging.edit` | the message as it will appear, with the channel name; a pending card shows the approval buttons |
| `messaging.delete` | the message to delete, in the danger tone |
| `messaging.react` | the emoji and the message it goes on |
| `messaging.mark_read` | one line: channel name and "marked read" |

A click on a message or channel opens it in the shared messaging screen by `ref`.

## 9. Contract tests

Every provider passes one suite (`messaging::contract::run`). A provider's crate gives it a function that makes a fresh
provider with one public channel and one dm, both empty.

1. `send` then `history` returns what was sent, newest first, and an empty text is `Invalid`.
2. A reply appears in `thread`, not in `history`; the root's `reply_count` grows; a reply to a reply lands under the root.
3. History pages do not repeat or skip a message while the data does not change.
4. `capabilities` is honest: each unlisted operation returns `Unsupported`, each listed one works.
5. `edit` changes the text and sets `edited_at`; `delete` removes the message from history and thread.
6. `react` twice by the same actor counts once; `react` off removes it.
7. The actor is kept, and an agent's message has `origin` `"<person>'s agent"`.
8. `subscribe` delivers each new message once, in order, and a dropped subscription stops.
9. A reference parses back and is of the form of section 2; an unknown channel or message is `NotFound`.
10. `channels` filters by kind and by text; `search` finds a message by a word of its text.
11. `mark_read` is accepted for a channel and refused with `NotFound` for an unknown one.

## 10. Open questions

1. **Reactions of an agent.** `react` asks every time, like `send`. A reaction is small and cheap to undo. Allow a
   per-channel "always allow reactions"?
2. **Origin on a service with one identity.** The origin line in the text is the only visible mark on Slack. Is a last
   italic line right, or should the provider use a service feature (a bot, an "app" label) when it has one?
3. **Direct messages and agents.** May an agent send a dm to a person who is not in a channel it was given? v1 says the
   app asks as for any send; a stricter rule could refuse.
4. **Presence and typing.** They are listed as features only. Add calls in v1.1, or leave them to the screen?
5. **Page type.** `messaging::Page` is the same shape as `tasks::Page`. Move one `Page<T>` to the crate root?
6. **Edits by others and history of edits.** v1 keeps only `edited_at`. Keep old texts?
