# Agents

atelier talks to agents through one model of its own. The app and the UI see events, commands and
capabilities. They never see an agent's wire format, its tool names or its process. Code lives in
`crates/agents`: `session` is the model, `claude_code` and `acp` are the backends that run an agent's
process, `own` is atelier's own agent, and `subprocess` is a helper for backends that run a child process.
`claude` and `cursor` hold what is particular to each agent: its launch, models and look.

Nothing outside `crates/agents` names an agent, a lab or a forge.

## The model

A session pushes **events** into a sink and takes **commands**. Both are plain data.

| Event | Says |
| --- | --- |
| `Started` | The session id, the model and the permission mode in force. |
| `UserMessage` | A user message atelier did not send: history, or another client. |
| `Text`, `Thinking` | Streamed deltas of a block. `ThinkingDone` carries the time it took. |
| `ToolStarted`, `ToolInput`, `ToolStatus`, `ToolFinished` | One tool call: its name, its `ToolKind`, its input, its file, its output. |
| `ToolEdit` | The text an edit or a write changes, as a `FileEdit` (path, old text, new text): told again as it grows while the agent writes the call. |
| `SubagentStarted`, `SubagentProgress`, `SubagentEnded` | A subagent: its task, kind and model. Its calls carry it as `parent`. |
| `Todos` | The whole todo list, each time it changes. |
| `Permission`, `PermissionCancelled` | A question with its tool call and its choices as data. |
| `Usage` | Tokens and cost of a turn. |
| `TurnEnded` | `Completed`, `Interrupted` or `Failed`, with the closing text. |
| `Warning` | A problem that does not end the session, such as a line that does not parse. |
| `Ended` | The session is over: closed by atelier, the process exited, or it failed. Nothing follows. |

Commands: `Send` (a text and its attachments), `Answer` (a request and one of its choices), `Interrupt`,
`SetModel`, `SetPermissionMode`.

An `Attachment` is a comment on lines of a file with the lines quoted, or a file. A backend that takes only
text writes them out with `message_text`. `crates/review` makes the comments (`docs/review.md`).

`ToolTarget { id, file }` names the file a call will touch as soon as the stream says so, before the call's
input is whole and before the tool runs. Claude Code sends the input as `input_json_delta` chunks; the
mapper reads the file out of the first top-level `file_path`, `notebook_path` or `path` key whose value has
closed. A review takes the file's text before the edit lands from this event.

`ToolEdit { id, edit }` is the one way the UI learns what an edit changes. Each backend fills the `FileEdit` from
its own words, and the UI reads only it and the call's `ToolKind`, never a tool's name or its input's keys:

| Backend | Where the edit comes from | Streams as it is written |
| --- | --- | --- |
| Claude Code | `Edit` (`file_path`, `old_string`, `new_string`) and `Write` (`content`), read from the `input_json_delta` chunks once the file has closed, and again from the whole input. `MultiEdit` has none. | Yes |
| Own agent | Each tool's `Tool::edit` (`edit`, `write`), from the Anthropic `input_json_delta` chunks or the OpenAI argument pieces once the path has closed, and again from the whole input. | Yes |
| ACP | The first `diff` in a call's content. A new file (no old text, or Cursor's `-- /dev/null`) has no old text. | No: Cursor sends the path while the call runs and the diff only when it is done (checked on 2026.10.01-14929f9). |

`crate::partial_json` reads the string fields of an input that is not whole yet, for every backend that gets it in
pieces. When `ToolEdit`s of one call come back to back in one frame, only the newest is kept.

A tool call names its tool as data. `ToolKind` (read, edit, write, search, shell, fetch, other) lets the
UI pick a look without knowing the agent's names. A permission request carries its choices, each with a
`ChoiceKind` (allow, allow always, deny). The UI draws whatever the agent offers.

`Capabilities` says what a backend supports: resume, interrupt, the models and permission modes a
session can switch to, thinking, subagents and todos. The UI hides what a backend lacks.

### The seam

```rust
trait Backend: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    fn open(&self, project: Arc<dyn Project>, request: OpenRequest, sink: EventSink)
        -> Result<Box<dyn Session>, SessionError>;
    fn sessions(&self, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError>;   // optional
    fn history(&self, project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError>; // optional
}
trait Session: Send {
    fn send(&self, command: Command) -> Result<(), SessionError>;   // never waits for the agent
}
```

The trait does not say "process", "stdio" or "JSON". `EventSink` is a closure, like the project's
`ChangeSink`, so a backend calls it from whatever thread it has. Dropping a session stops the agent.
`open` does not block on the agent: `Started` says when it is ready.

Nothing blocks the UI thread. The app calls `open`, `sessions` and `history` from a background task.
`EventQueue` is the sink the app gives a session. It joins the deltas of one block while events wait
and wakes the UI once when the queue goes from empty to not empty, so a stream costs one repaint a
frame. The UI calls `drain` once per frame.

`FakeBackend` (tests only) answers each message with scripted events in the caller's thread. It has no
process and no thread, and the model's tests run on it. It shows that the trait fits a backend that lives
in atelier.

## Three kinds of backend

1. **A subprocess with its own protocol.** Claude Code, over stream-json. It runs through
   `Project::spawn`, so a remote project runs it on its host. Built.
2. **An ACP agent.** One generic backend for every agent that speaks the Agent Client Protocol.
   Built, with Cursor as its first agent. See below.
3. **Our own agent, in process.** An agent loop in atelier that calls model APIs and runs its tools through
   `Project`. Not built. See "Our own agent".

Kinds 1 and 2 share `subprocess`: start a child through the project, read its stdout by line, write to
its stdin. Kind 3 does not use it.

## The Claude Code backend

Checked against `claude` 2.1.284, logged in, on the HP. The captured runs are the fixtures in
`crates/agents/tests/fixtures/claude_code/`. Each run is one `claude` process; hook and rate limit lines
are removed, and paths are made generic.

### Start

```
claude --print --input-format stream-json --output-format stream-json --verbose
       --include-partial-messages --permission-prompts host --permission-prompt-tool stdio
       [--resume <id>] [--model <m>] [--permission-mode <mode>]
```

`--permission-prompts host` alone does not work. `claude` then denies anything that needs a question and
writes a `tool_result` that starts with "Claude requested permissions to". The hidden flag
`--permission-prompt-tool stdio` makes it ask over stdio. It is not in `--help`.

`Ask` is `claude`'s default, so atelier passes no `--permission-mode` for it.

### What atelier writes (one JSON object per line)

| Line | When |
| --- | --- |
| `{"type":"user","message":{"role":"user","content":"<text>"}}` | Send a message. |
| `{"type":"control_request","request_id":"atelier-1","request":{"subtype":"interrupt"}}` | Interrupt. |
| `... {"subtype":"set_model","model":"opus"}` | Set the model. |
| `... {"subtype":"set_permission_mode","mode":"acceptEdits"}` | Set the mode. |
| `{"type":"control_response","response":{"subtype":"success","request_id":"<id>","response":{"behavior":"allow","updatedInput":<input>}}}` | Allow a tool. `updatedPermissions` carries the rules `claude` suggested for "always allow". |
| the same with `"behavior":"deny","message":"..."` | Deny a tool. |

Checked live: `set_model` and `set_permission_mode` each get a `control_response` with `success`. A
model switch takes effect on the next turn and writes a second `system/init` line, so `Started` arrives
again with the new model.

### What `claude` writes

| Line | atelier's event |
| --- | --- |
| `system/init` (`session_id`, `model`, `permissionMode`) | `Started` |
| `stream_event`: `message_start` | Remembers the message id, so its finished copy does not repeat the text. |
| `stream_event`: `content_block_start` (`text`, `thinking`, `tool_use`) | `Text`, `Thinking` (empty, "the agent thinks now"), `ToolStarted` with no input yet. |
| `stream_event`: `content_block_delta` (`text_delta`, `thinking_delta`) | `Text`, `Thinking`. Empty deltas are dropped. |
| `stream_event`: `content_block_stop` | `ThinkingDone` with the time atelier measured. |
| `stream_event`: `content_block_delta` (`input_json_delta`) | `ToolTarget` once the file key has closed, then `ToolEdit` for an `Edit` or a `Write` each time its text grows. |
| `assistant` (one line per finished block) | `ToolInput` for a streamed call, and `ToolEdit` for an `Edit` or a `Write`. Text and thinking only when the message did not stream (a transcript). |
| `user` with a `tool_result` | `ToolFinished`. |
| `control_request` `can_use_tool` | `Permission` |
| `control_cancel_request` | `PermissionCancelled` |
| `system/task_started`, `task_progress`, `task_notification` of `task_type` `local_agent` | `SubagentStarted`, `SubagentProgress`, `SubagentEnded` |
| `system/task_started` and `task_notification` of `task_type` `local_bash` (a shell command run with `run_in_background`) | No event of their own: the shell call stays `Running` and `ToolFinished` comes at the notification |
| `result` | `Usage`, then `TurnEnded` |
| `system/status`, `thinking_tokens`, `hook_*`, `rate_limit_event`, `control_response`, anything new | Ignored. |

Things the captures showed, which the docs do not say:

- **Thinking has no text** on the models tried (`thinking_display: "updates"`). atelier measures the time
  from `content_block_start` to `content_block_stop` itself.
- **A permission answer is a `control_response` to the request's id.** After an interrupt during a
  question, `claude` sends `control_cancel_request` for it.
- **Interrupt** gets a `control_response` (`still_queued`), a user text "[Request interrupted by user
  for tool use]" (dropped, it is not something the user said), then `result` with subtype
  `error_during_execution` and `terminal_reason: "aborted_tools"`. atelier maps that to
  `TurnOutcome::Interrupted`, and finishes any call still open as a failed one first.
- **A large tool output is saved to a file.** The `tool_result` text is
  `<persisted-output> Output too large (340.7KB). Full output saved to: <path> Preview (first 2KB): ...`.
  atelier passes on the preview and the path (`ToolOutput::full_at`). Any other output over 64 KiB is cut
  to its head on a character boundary and marked `truncated`.
- **Todos are `TaskCreate` and `TaskUpdate` calls** in 2.1.284 (`TodoWrite` in older versions). atelier
  keeps the list and sends `Todos`. It does not show these calls as tool calls.
- **`Agent` is the tool that starts a subagent** (`Task` in older versions). A subagent's own calls are
  `assistant` and `user` lines with `parent_tool_use_id`. A subagent can run in the background: the
  `Agent` result then says it launched, and the turn can end before the subagent does. `task_notification`
  (with `status` and `summary`) ends it in both cases.
- **A background shell command is a shell call that keeps running, not a subagent.** `claude` announces it with
  `task_started` (`task_type` `local_bash`) after the call has returned "Command running in background".
  atelier keeps the call `Running`, drops that first result, and ends it with `ToolFinished` when
  `task_notification` comes, with the notification's summary (an error when the status is not `completed`).
  A turn that ends does not end it; a process that exits does. Only a task of type `local_agent` (or one
  with no type) is a subagent.
- **History ends what the record left open.** A transcript has no end for a subagent or a call that was
  still going when it stopped being written, and no live agent will send one. So `history` closes each one
  at the end: `SubagentEnded` and `ToolFinished` as finished, with a note that the record has no end for it,
  or as interrupted (`ok: false`, an error result) when the last turn ended aborted. A subagent whose
  notification is in the record ends there, once.
- **A resumed session does not replay its history** on stdout. atelier reads it from `claude`'s own record.

### Sessions

`claude` keeps each session in `~/.claude/projects/<folder>/<id>.jsonl`, where `<folder>` is the
project's root with every character other than a letter or digit as `-`. The file is on the host, so
atelier reads it through a process the project spawns (`sh`), never from disk directly.

- `sessions`: the 50 newest files, each with its id, its modification time and the first user message
  (commands and hook notes skipped, cut to 100 characters).
- `history`: the whole file, through the same mapper. A transcript has no streaming, so text comes
  whole and a thinking block has no time. Lines marked `isSidechain` or `isMeta` are skipped.
- Resume is `--resume <id>`. Live test: a resumed session answers a question about its first turn.

### Failure

- **`claude` is not on the host:** `open` returns `SessionError::Missing { program }`.
- **The process exits mid-turn:** the reader hits the end of stdout, waits for the exit code, and the mapper
  finishes what is open: each running call fails, each waiting question is cancelled, each subagent ends,
  then `TurnEnded(Failed("the agent exited with code N: <its last stderr line>"))` and `Ended(Exited { code, stderr })`, where `stderr` is the last 20 lines the process wrote. `Ended` comes once.
- **A line does not parse:** a `Warning`. The stream goes on.
- **atelier dies** (a crash, a kill): every process a local project starts has a watchdog (`sh`, detached)
  that stops it, and kills it two seconds later if it still runs, once atelier is gone. An agent that ignores
  the end of its stdin, as Cursor's does, ends too. On a remote project `atelier-remote` kills its processes
  when the app's connection closes.
- **atelier closes the session:** dropping it closes stdin, kills the process and sends `Ended(Closed)` at
  once. The process can have children that keep its pipes open, so atelier does not wait for the end of
  stdout.
- `Project::spawn` drops stderr, so a crash carries an exit code and no message. Open item.

### Threads

Two threads per session: the reader (lines to events, into the sink) and the writer (queued lines to
stdin). `send` only queues, so it never waits on a full pipe.

## The ACP backend

Agent Client Protocol: JSON-RPC 2.0 over the agent's stdin and stdout. One backend, `Acp`
(`crates/agents/src/acp`), serves every agent that speaks it. An `AcpAgent` says how to start one, which
of the agent's modes stand for atelier's permission modes, and which models to offer. Everything else comes
over the protocol. The agent starts through `Project::spawn`, on the host.

| atelier | ACP |
| --- | --- |
| `open` | `initialize`, then `session/new`, or `session/load` to resume when the agent says it can. On error `-32000` (sign-in needed) atelier calls `authenticate` once with the agent's first method and asks again. |
| `Command::Send` | `session/prompt`, one turn at a time. Messages sent during a turn wait. Its response carries the stop reason and ends the turn. |
| `Command::Interrupt` | `session/cancel` (a notification). A waiting question is answered `cancelled`. The turn ends with stop reason `cancelled`. |
| `SetPermissionMode` | `session/set_mode` with the agent's name for the mode. A mode it has no name for is `Unsupported`. |
| `SetModel` | The agent's `model` config option when it has one, else `session/set_model` (unstable). A model given by its name is set by the full id the agent listed. |
| `Text`, `Thinking` | `agent_message_chunk`, `agent_thought_chunk`. |
| `ToolStarted`, `ToolInput`, `ToolTarget`, `ToolFinished` | `tool_call` and `tool_call_update`, merged by id. The kind maps to `ToolKind` (read, edit, search, execute as shell, fetch; delete and move as edit; others as other). An edit whose diff has no old text is a write. Locations and diffs give `file`, and a diff gives `ToolEdit`. |
| `Todos` | `plan` updates: the whole list each time. |
| `Permission` | The agent's request `session/request_permission`. Its options become `Choice`s, the call's content is the reason, and `Answer` is the response. |
| `Started` | The session's id, mode and model, again each time the agent changes them (`current_mode_update`, `config_option_update`) or names its commands (`available_commands_update`). |
| `sessions`, `history` | `session/list`, and `session/load` in a short-lived process, whose replayed updates are the history. |

atelier tells the agent it serves no files and no terminal (`fs` and `terminal` are false), so the agent
runs its tools itself. Any other request from the agent gets "method not found". Stop reasons other than
`end_turn` and `cancelled` fail the turn in atelier's words.

### Cursor

`agent acp`, Cursor's CLI, checked on 2026.10.01-14929f9, logged in with `agent login`, on the HP. The
captured runs are the fixtures in `crates/agents/tests/fixtures/cursor/`, and `acp/tests/replay.rs` plays
them back. `crates/agents/tests/cursor_live.rs` runs the real CLI end to end:
`cargo test -p atelier-agents --test cursor_live -- --ignored --nocapture`.

- **Sign-in:** none needed once logged in, though `initialize` still offers `cursor_login`.
- **Modes:** `agent`, `plan` and `ask`. atelier's Ask is `agent`, where a command outside Cursor's allowlist
  asks first, and Plan is `plan`. `ask` (read only) has no atelier mode and is not offered.
- **Models:** in `models.availableModels`, with options in the id: `default[]` (Auto),
  `composer-2.5[fast=true]`. `session/set_model` takes only the full id, so atelier shows the name before
  `[` and sets the full id. The list atelier offers is in `cursor.rs`.
- **Tools:** a read's text is in `rawOutput.content`, a command's in `rawOutput.stdout` and `stderr`, an
  edit is a diff, and a new file's diff has old text `-- /dev/null`.
- **Questions:** a question comes after the call is running. Its content is the reason ("Not in
  allowlist: rm"), not output. A denied call ends `completed` with no output.
- **Plan mode:** a "Create Plan" tool, then a `plan` update, then Cursor's own request
  `cursor/create_plan`, which atelier declines.
- **Load:** replays each user message as one chunk with no message id, so chunks join only under one id.
- **List:** `updatedAt` is an ISO time with milliseconds.
- **Usage:** Cursor reports no tokens, so its turns have no `Usage`.
- **Processes:** `agent acp` starts a `worker-server` for the folder, which outlives it and the next run
  in that folder reuses. Cursor's own `agent -p` leaves it too. After a turn, `agent acp` does not exit
  when its stdin closes. So atelier ties every local child to itself (see "Failure" above).

Other agents that speak ACP (Gemini CLI, opencode, Codex through an adapter) are an `AcpAgent` each, with
no change in `acp` or `session`. Their launch and modes are to be checked against the real agent first.

## Our own agent

atelier's own agent runs in process. There is no child and no wire format. It is a `Backend` like the
others, and it is the case the trait was shaped for. It is built (`crates/agents/src/own`); this is the design,
and "What is built" below says where the build differs.

```
Session::send ──> loop thread ──> model client ──> API (Anthropic, OpenAI-compatible, OpenRouter)
                      │  ^
                      │  └── tool results
                      ├──> tools ──> Project (read, write, search, spawn)
                      └──> permissions ──> Event::Permission ... Command::Answer
```

**The loop.** One thread per session. It owns the conversation. A `Command::Send` adds a user message
and runs turns: call the model, stream its reply as `Text` and `Thinking`, and when the reply holds tool
calls, run them and call the model again. It stops on an end of turn, a failure or an interrupt, and then
sends `TurnEnded`. `Command::Interrupt` sets a flag that the model stream and each tool check; a tool that
runs a process kills it.

**The model client.** A small trait, `Model`, with one method: given the conversation, the tool
definitions and a sink, stream a reply as text deltas, thinking deltas and tool calls, and return the
usage. Three implementations to start: Anthropic's Messages API, any OpenAI-compatible chat API, and
OpenRouter, which is the second with a base URL and a header. The client does its own HTTP off the UI
thread. It knows no `Project` and no tools; it only knows messages. The model choice is a `ModelChoice`
in `Capabilities`, so switching is `SetModel`.

**The tools.** One file per tool, each a function of `&dyn Project` and its input: read, write, edit,
search, shell. Each has a `ToolKind`, a JSON schema for the model and a run function. They call only
`Project`: `read`, `write`, `list`, `search` and `spawn`. So the agent works on a local folder and on an
SSH host with no other code. A shell call is `Project::spawn` of `sh -c`, with its output streamed into
`ToolFinished` and cut to `ToolOutput::MAX_TEXT`. Edit is an exact-string replace, and it fails when the
string is missing or not unique, as the model must see that failure.

**Permissions.** Each tool says what a call does: it reads, it changes files, it runs a process. A
`PermissionMode` and a rule list decide: allow, ask or deny. `Ask` sends `Event::Permission` with the tool
call and the choices (allow, always allow, deny), and the loop waits on a channel that `Command::Answer`
feeds. `Plan` allows only reads. `AcceptEdits` allows file changes. `Bypass` allows all. "Always allow"
adds a rule that atelier keeps in its own settings. This is atelier's own logic and does not depend on a
backend.

**What it gives the UI, through the same events.** Streamed text and thinking with its time, tool calls
with kind, input, file and output, a todo tool that sends `Todos`, subagents (a tool that starts a second
loop and sends `SubagentStarted` and `SubagentEnded`, with its calls under it as `parent`), usage per turn
with the cost, and `Capabilities` that say it supports all of it.

**Sessions.** atelier keeps the conversation in its own record, one JSON-lines file per session under the
project's settings folder, written through `Project::write`. `sessions` lists them, `history` maps them to
events, and resume loads the conversation back into the loop.

**Why the trait needs nothing more.** The loop pushes the same events, takes the same commands and
returns the same errors. `open` starts the loop thread and returns at once. The fake backend in the tests
already works this way: no process, events made in the caller's or its own thread.

### What is built
The module is `atelier_agents::own`. `OwnAgent` is the `Backend` (name `atelier`).
| File | What it does |
| --- | --- |
| `message.rs` | The conversation as blocks (`Text`, `Thinking` with its signature, `Redacted`, `ToolUse`, `ToolResult`), the `Model` trait, `Cancel`, `Secret`, `ModelError`. |
| `http.rs` | A small HTTP/1.1 client (plain and TLS through rustls). Every wait wakes ten times a second to look at the cancel flag, so an interrupt reaches a stalled model at once and closes the connection. No header of a request is ever in an error. |
| `sse.rs` | Server-sent events from bytes cut anywhere. |
| `anthropic.rs` | The Messages API, streaming. Tool use, adaptive thinking (a token budget for Haiku 4.5), and prompt caching. `models()` lists the account's models from `GET /v1/models`. |
| `openai.rs` | Chat Completions, streaming, for OpenAI, OpenRouter and any server that copies it. Reasoning arrives as `reasoning_content` or `reasoning`. |
| `tools.rs`, `tools/*` | `read`, `list`, `search`, `edit`, `write`, `shell`. |
| `permission.rs` | The decision table. |
| `context.rs` | The context budget. |
| `store.rs` | The session record, `sessions`, `history`. |
| `runner.rs` | The loop thread and the `Session` handle. |
**Keys.** `OwnAgent::from_env()` reads `ANTHROPIC_API_KEY`, then `OPENROUTER_API_KEY`, then `OPENAI_API_KEY`. The
constructors `anthropic(key)`, `openai(key)` and `openrouter(key)` take a `Secret` from the settings. With no key
the agent still exists, and `open` says what to set. A `Secret` prints as `***`. The key is sent in one header, is
never in an event, an error, a log, the record of a session or the repository, and a test checks each.
**The request.** Three cache breakpoints (the last tool, the system prompt, the end of the conversation), adaptive
thinking with `display: "summarized"` so the reasoning can be shown, no sampling settings, no forced `tool_choice`,
no prefill, and `eager_input_streaming` on every tool, so the input of a call streams as it is made (a
file to write shows up as it is written). The API does not validate such an input, so one that is cut off
or is not JSON is marked malformed and the model is told. The system prompt holds nothing that changes from call to call, so the cache keeps it. Thinking blocks
go back signed, as the API needs them in a turn that used a tool. A reply cut off by the token limit, or refused,
keeps no tool call that has no result.
**Retry.** A rate limit (429), a server failure (5xx, 529) and a dropped connection are tried again, up to
`RetryPolicy::max_retries` times (three), after a wait that doubles from a second up to 30 s, or the
`retry-after` the API gave. Only when nothing of the reply has been shown: a reply that broke halfway is a
failed turn, not a repeat of text. A refused key, a bad request and a missing model are not retried.
**Tools and events.** A call emits `ToolStarted` (kind, `Pending`) as the model names it, `ToolInput` and
`ToolTarget` when its input is whole (before the tool runs), `ToolEdit` for `edit` and `write` while the input streams
and when it is whole, `ToolStatus` `Running`, and `ToolFinished`. `list`
and `search` are kind `Search`. Paths are checked to be inside the project. `write` makes the folder. `edit` fails
when the text is missing or appears more than once. `shell` runs `sh -c` with stderr joined to stdout, stops at
`timeout_secs` (120, at most 600) or an interrupt, kills the command's whole process tree (through the project, so
it works over SSH), and keeps 256 KiB of output. Results reach the model cut to 30,000 bytes and the UI cut to
`ToolOutput::MAX_TEXT`.
**Permissions.** `Ask`: reads run, changes and commands ask. `AcceptEdits` and `Auto`: reads and file changes run,
commands ask (this agent has no judge of its own, so `Auto` is `AcceptEdits`). `Plan`: reads only; the rest is
refused with words the model can act on. `Bypass`: all run. "Always allow" adds a rule for the tool for the rest of
the session (`OwnOptions::allow` starts a session with rules the settings keep). A denied call is a tool error the
model reads, and the turn goes on. An interrupt while a question waits cancels it.
**Context.** Past `Budget::limit_tokens` (120,000, estimated at four bytes a token) the oldest tool results, and
the big strings in old tool inputs (a file that was written), shrink to a note and their first 240 bytes, oldest
first, until the conversation is under 80% of the limit. The newest eight messages stay whole. The shape of the
conversation stays. A shortened message changes the cache from that point, so it happens only when the budget
is passed, and it says so in a `Warning`.
**Sessions.** The record is kept in the project's data folder, through `Project::data_write`, outside the repository
and on the project's host: `agent/sessions/<id>.jsonl` (one message a line) and `<id>.meta` (title, model, time).
`sessions` lists them with `data_list` (newest first by the time in the meta, at most 50), `history` maps them to
events, and resume loads the messages back. An id is letters, digits and dashes, so it is never a path. A save
that fails warns once and the session goes on. Both files are rewritten whole after each message.
**Where the record lives.** In the data folder (`crates/project/src/data.rs`): under the app's data folder on this
machine (`<data>/atelier/projects/<name>-<hash of the root>/`), and on the project's host over SSH. It never shows in
`git status`, and atelier writes no git file for it. An older atelier wrote it in the project's own `.atelier/agent/sessions/`.
The first time the list is read, or a session is asked for by id, `store::migrate` moves what it finds there into the
data folder and removes the old files and their empty folders. A session that is in both places keeps the data
folder's copy; one with no messages is left where it is.
**Interrupt.** `Command::Interrupt` sets a flag that the HTTP read, the retry wait, the permission wait and a
running command all look at. What streamed before it stays in the conversation as text. A message sent while a
turn runs waits and runs next.
**Not built.** Subagents and a todo tool (`Capabilities` says so), a cost in dollars (`Usage::cost_usd` is
`None`), a persistent "always allow" (the settings keep the list; `OwnOptions::allow` takes it), and tool
calls run in parallel (they run one after another).
**To add it to the registry** (`registry.rs` is the app's): one entry in `agents()`,
`Agent { backend: Arc::new(OwnAgent::from_env()), name: "atelier", mark: None, look: claude::look(), lab: Lab::Anthropic }`.
`mark: None` gives the monogram. The look is a stand-in until atelier has its own.
**Tests.** `crates/agents/src/own/tests` play scripted streams from a local HTTP server: text, thinking, one tool,
several tools, a tool error, permission asked, allowed, denied and always allowed, plan mode, an interrupt in
a stream, at a question and in a command, a malformed and a truncated stream, a 429 that is retried, a server that
keeps failing, a refused key, a cut-off reply, a refusal, resume, and a cancelled request. No default test calls a
real API. `crates/agents/tests/own_live.rs` has two tests that do, ignored:
`ANTHROPIC_API_KEY=... cargo test -p atelier-agents --test own_live -- --ignored --nocapture --test-threads=1`.
They use Haiku 4.5 and cost a few cents. `crates/agents/tests/own_perf.rs` measures the numbers in
`docs/performance.md`.
## Performance

Targets and numbers are in `docs/performance.md` ("Agent sessions"). The measurements are
`crates/agents/tests/perf.rs`.

## Open

- `Project::spawn` drops stderr, so a crash has no message. The interface needs a way to keep it.
- `Backend::sessions` and `history` start `sh` on the host. A host with no POSIX shell has no list.
- Cursor reports no tokens, so a Cursor turn has no `Usage`.
