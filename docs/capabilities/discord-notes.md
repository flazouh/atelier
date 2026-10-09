# The Discord provider: notes

The provider is the crate `atelier-discord`. It follows `messaging-v1.md` and runs `discordcli`
(github.com/flazouh/discordcli) for every call. It has no login of its own.

## Read-only by default, and why

`discordcli` signs in as the person, with the person's own Discord user token, taken from a browser session. Discord's
terms do not allow automation of a user account. A bot account is the allowed way, and `discordcli` is not one. Actions
by a program on a user token can get the account limited or banned. Reading is low risk. Sending is the action that
people notice and report.

So the provider reads by default:

- `DiscordConfig::allow_writes` is `false` unless the app sets it. With `false`, `send` runs no command and returns
  `Provider { code: "read_only" }`. It stays in the core list of operations because the contract needs it there, so the provider also lists the
  `read_only` feature (exactly when `allow_writes` is `false`) and the screen and the agent tools hide what writes.
- Every write the app asks for still goes through the "ask every time" rule of the spec (section 8). The flag is a second
  lock. The app should set it only after the person turns it on for one account, knowing this risk.
- Every look of a subscription is a request on the same token. The default is one look every 15 seconds for each channel,
  and a subscription for more than `max_polled` (20) channels is refused. A screen that wants "everything" must name
  channels.

## What the provider maps

| Capability | Discord | Notes |
|---|---|---|
| account | a server id, or `dm` | `DiscordConfig::include_dms` lets a server account also hold the `dm` channels |
| `channels` | `channels`, `dms` | text, announcement and thread channels, dms and group dms. No category, voice, stage or forum |
| `history` | `read` | oldest-first rows turned newest first; the cursor is the oldest id read (`--before`) |
| `thread` | `read --after` | see below |
| `send`, reply | `send`, `reply` | needs `allow_writes` |
| `search` | `search` | one server; not listed when the provider also holds dms |
| `subscribe` | `read --after`, polled | replies included |

References are `messaging:discord:<server-id>:<channel-id>:<message-id>`, and `messaging:discord:dm:<channel-id>:...`
for a dm. Every id is a Discord id (17 to 20 digits). An id of another form is `NotFound` and runs no command.

### Replies and threads

Discord has two things the spec calls a thread. A **reply** is a message in the channel that points at another message.
A **thread channel** is a channel of its own. The provider maps replies, because that is what a person sees under a
message. So:

- `history` hides replies and gives a root the number of replies found **in the same page**. A reply that is on a newer
  page is not counted.
- `thread` gives the root and every message after it that answers it, or answers one of its replies. It reads up to 500
  messages after the root and stops there.
- A reply to a reply goes under the root, as the spec says. `send` with `in_thread_of` replies to the message named, so
  Discord shows the quote of that message, and returns the root as `parent`.
- A Discord thread channel is a channel here: its id works in `history`, `send` and `subscribe`. `channels` does not list
  it, because the tool lists no thread channel.

## What `discordcli` cannot do

These are not listed in `capabilities()`. A call returns `Unsupported` and runs no command.

- `edit`, `delete`, `react`, `mark_read`: the tool has no command.
- `person`: the tool has no user lookup. A message carries the author's id and display name, and that is all.
- `export` and `import`: no provider has them in v1.
- Reactions, edit times, `unread`, `topic`, `member_count`: the rows do not carry them. `edited_at` is always empty.
- Search in a dm, and search in more than one server.
- Starting a thread channel, and listing threads.
- Sending a file.

## Text

Discord reads nearly the markdown subset of the spec. The provider changes three things.

- A link `[text](url)` goes out as `text (url)`, because a message from a person does not show link text.
- A link to a person, a channel or a message of Discord goes out as `<@id>`, `<#id>` or the message address.
- No text can ping a crowd. `@everyone`, `@here` (in any case) and `<@...>` that a sender typed get a zero-width space, so
  Discord does not read them. `discordcli` also tells Discord to parse no mention, so this is the second lock. Code is
  copied as written.

An agent's message ends with `_sent by <person>'s agent_` and has `origin` set. A message over 2000 UTF-16 units, with the
origin line, is `Invalid`.
