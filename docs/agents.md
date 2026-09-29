# Agents

lathe talks to agents through one model of its own. The app and the UI see events, commands and
capabilities. They never see an agent's wire format, its tool names or its process. Code lives in
`crates/agents`: `session` is the model, `claude_code` is the first backend, `subprocess` is a helper
for backends that run a child process.

Nothing outside `crates/agents` names an agent, a lab or a forge.

## The model

A session pushes **events** into a sink and takes **commands**. Both are plain data.

| Event | Says |
| --- | --- |
| `Started` | The session id, the model and the permission mode in force. |
| `UserMessage` | A user message lathe did not send: history, or another client. |
| `Text`, `Thinking` | Streamed deltas of a block. `ThinkingDone` carries the time it took. |
| `ToolStarted`, `ToolInput`, `ToolStatus`, `ToolFinished` | One tool call: its name, its `ToolKind`, its input, its file, its output. |
| `SubagentStarted`, `SubagentProgress`, `SubagentEnded` | A subagent: its task, kind and model. Its calls carry it as `parent`. |
| `Todos` | The whole todo list, each time it changes. |
| `Permission`, `PermissionCancelled` | A question with its tool call and its choices as data. |
| `Usage` | Tokens and cost of a turn. |
| `TurnEnded` | `Completed`, `Interrupted` or `Failed`, with the closing text. |
| `Warning` | A problem that does not end the session, such as a line that does not parse. |
| `Ended` | The session is over: closed by lathe, the process exited, or it failed. Nothing follows. |

Commands: `Send` (a text and its attachments), `Answer` (a request and one of its choices), `Interrupt`,
`SetModel`, `SetPermissionMode`.

An `Attachment` is a comment on lines of a file with the lines quoted, or a file. A backend that takes only
text writes them out with `message_text`. `crates/review` makes the comments (`docs/review.md`).

`ToolTarget { id, file }` names the file a call will touch as soon as the stream says so, before the call's
input is whole and before the tool runs. Claude Code sends the input as `input_json_delta` chunks; the
mapper reads the file out of the first top-level `file_path`, `notebook_path` or `path` key whose value has
closed. A review takes the file's text before the edit lands from this event.

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
in lathe.

## Three kinds of backend

1. **A subprocess with its own protocol.** Claude Code, over stream-json. It runs through
   `Project::spawn`, so a remote project runs it on its host. Built.
2. **An ACP agent.** One generic backend for every agent that speaks the Agent Client Protocol.
   Not built. See below.
3. **Our own agent, in process.** An agent loop in lathe that calls model APIs and runs its tools through
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

`Ask` is `claude`'s default, so lathe passes no `--permission-mode` for it.

### What lathe writes (one JSON object per line)

| Line | When |
| --- | --- |
| `{"type":"user","message":{"role":"user","content":"<text>"}}` | Send a message. |
| `{"type":"control_request","request_id":"lathe-1","request":{"subtype":"interrupt"}}` | Interrupt. |
| `... {"subtype":"set_model","model":"opus"}` | Set the model. |
| `... {"subtype":"set_permission_mode","mode":"acceptEdits"}` | Set the mode. |
| `{"type":"control_response","response":{"subtype":"success","request_id":"<id>","response":{"behavior":"allow","updatedInput":<input>}}}` | Allow a tool. `updatedPermissions` carries the rules `claude` suggested for "always allow". |
| the same with `"behavior":"deny","message":"..."` | Deny a tool. |

Checked live: `set_model` and `set_permission_mode` each get a `control_response` with `success`. A
model switch takes effect on the next turn and writes a second `system/init` line, so `Started` arrives
again with the new model.

### What `claude` writes

| Line | lathe's event |
| --- | --- |
| `system/init` (`session_id`, `model`, `permissionMode`) | `Started` |
| `stream_event`: `message_start` | Remembers the message id, so its finished copy does not repeat the text. |
| `stream_event`: `content_block_start` (`text`, `thinking`, `tool_use`) | `Text`, `Thinking` (empty, "the agent thinks now"), `ToolStarted` with no input yet. |
| `stream_event`: `content_block_delta` (`text_delta`, `thinking_delta`) | `Text`, `Thinking`. Empty deltas are dropped. |
| `stream_event`: `content_block_stop` | `ThinkingDone` with the time lathe measured. |
| `assistant` (one line per finished block) | `ToolInput` for a streamed call. Text and thinking only when the message did not stream (a transcript). |
| `user` with a `tool_result` | `ToolFinished`. |
| `control_request` `can_use_tool` | `Permission` |
| `control_cancel_request` | `PermissionCancelled` |
| `system/task_started`, `task_progress`, `task_notification` | `SubagentStarted`, `SubagentProgress`, `SubagentEnded` |
| `result` | `Usage`, then `TurnEnded` |
| `system/status`, `thinking_tokens`, `hook_*`, `rate_limit_event`, `control_response`, anything new | Ignored. |

Things the captures showed, which the docs do not say:

- **Thinking has no text** on the models tried (`thinking_display: "updates"`). lathe measures the time
  from `content_block_start` to `content_block_stop` itself.
- **A permission answer is a `control_response` to the request's id.** After an interrupt during a
  question, `claude` sends `control_cancel_request` for it.
- **Interrupt** gets a `control_response` (`still_queued`), a user text "[Request interrupted by user
  for tool use]" (dropped, it is not something the user said), then `result` with subtype
  `error_during_execution` and `terminal_reason: "aborted_tools"`. lathe maps that to
  `TurnOutcome::Interrupted`, and finishes any call still open as a failed one first.
- **A large tool output is saved to a file.** The `tool_result` text is
  `<persisted-output> Output too large (340.7KB). Full output saved to: <path> Preview (first 2KB): ...`.
  lathe passes on the preview and the path (`ToolOutput::full_at`). Any other output over 64 KiB is cut
  to its head on a character boundary and marked `truncated`.
- **Todos are `TaskCreate` and `TaskUpdate` calls** in 2.1.284 (`TodoWrite` in older versions). lathe
  keeps the list and sends `Todos`. It does not show these calls as tool calls.
- **`Agent` is the tool that starts a subagent** (`Task` in older versions). A subagent's own calls are
  `assistant` and `user` lines with `parent_tool_use_id`. A subagent can run in the background: the
  `Agent` result then says it launched, and the turn can end before the subagent does. `task_notification`
  (with `status` and `summary`) ends it in both cases.
- **A resumed session does not replay its history** on stdout. lathe reads it from `claude`'s own record.

### Sessions

`claude` keeps each session in `~/.claude/projects/<folder>/<id>.jsonl`, where `<folder>` is the
project's root with every character other than a letter or digit as `-`. The file is on the host, so
lathe reads it through a process the project spawns (`sh`), never from disk directly.

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
- **lathe closes the session:** dropping it closes stdin, kills the process and sends `Ended(Closed)` at
  once. The process can have children that keep its pipes open, so lathe does not wait for the end of
  stdout.
- `Project::spawn` drops stderr, so a crash carries an exit code and no message. Open item.

### Threads

Two threads per session: the reader (lines to events, into the sink) and the writer (queued lines to
stdin). `send` only queues, so it never waits on a full pipe.

## The ACP backend

Agent Client Protocol: JSON-RPC 2.0 over the agent's stdin and stdout. One backend, `Acp`, serves every
agent that speaks it, so adding one is a launch command in settings, not new code. This section is a plan
from the protocol's specification. It is not built and not checked against a real agent yet.

| lathe | ACP |
| --- | --- |
| `open` | `initialize`, then `session/new` (or `session/load` to resume when the agent says it can). |
| `Command::Send` | `session/prompt`. Its response carries the stop reason and ends the turn. |
| `Command::Interrupt` | `session/cancel` (a notification). The turn ends with stop reason `cancelled`. |
| `SetPermissionMode`, `SetModel` | `session/set_mode`, and the agent's model option, when it offers them. |
| `Text` | `session/update` with `agent_message_chunk`. |
| `Thinking` | `session/update` with `agent_thought_chunk`. |
| `ToolStarted`, `ToolFinished` | `tool_call` and `tool_call_update`. ACP gives a kind (read, edit, delete, move, search, execute, think, fetch, other), which maps to `ToolKind`, and locations, which give `file`. |
| `Todos` | `plan` updates: the whole list each time. |
| `Permission` | The agent's request `session/request_permission`. Its options (allow once, allow always, reject once, reject always) become `Choice`s, and `Answer` is the response. |
| `Capabilities` | From the `initialize` response, so the UI shows only what that agent offers. |

ACP is a two-way protocol: the agent calls the client. lathe must serve `fs/read_text_file` and
`fs/write_text_file`, and the `terminal/*` methods, and it serves them through `Project`. So an ACP agent
works on a remote project the same way Claude Code does. The agent process itself starts through
`Project::spawn`, on the host.

How the agents named so far plug in. The launch commands are to be checked before building:

- **Gemini CLI:** ACP mode of `gemini`.
- **opencode:** `opencode acp`.
- **Codex:** through an ACP adapter for Codex, since Codex has its own protocol.

None of them needs a change in `session`. If ACP has an event the model lacks, the model grows a
variant, and every backend and the UI keep working because the UI ignores what it does not know.

## Our own agent

lathe's own agent runs in process. There is no child and no wire format. It is a `Backend` like the
others, and it is the case the trait was shaped for. It is not built. This is the design.

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
adds a rule that lathe keeps in its own settings. This is lathe's own logic and does not depend on a
backend.

**What it gives the UI, through the same events.** Streamed text and thinking with its time, tool calls
with kind, input, file and output, a todo tool that sends `Todos`, subagents (a tool that starts a second
loop and sends `SubagentStarted` and `SubagentEnded`, with its calls under it as `parent`), usage per turn
with the cost, and `Capabilities` that say it supports all of it.

**Sessions.** lathe keeps the conversation in its own record, one JSON-lines file per session under the
project's settings folder, written through `Project::write`. `sessions` lists them, `history` maps them to
events, and resume loads the conversation back into the loop.

**Why the trait needs nothing more.** The loop pushes the same events, takes the same commands and
returns the same errors. `open` starts the loop thread and returns at once. The fake backend in the tests
already works this way: no process, events made in the caller's or its own thread.

## Performance

Targets and numbers are in `docs/performance.md` ("Agent sessions"). The measurements are
`crates/agents/tests/perf.rs`.

## Open

- `Project::spawn` drops stderr, so a crash has no message. The interface needs a way to keep it.
- `Backend::sessions` and `history` start `sh` on the host. A host with no POSIX shell has no list.
- The ACP backend and our own agent are designs only.
