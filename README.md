# atelier

A native code editor on GPUI, made for work with coding agents. It runs agent sessions next to your code,
shows what each turn changed for review, and opens pull requests, on a local folder or over SSH.

Agents plug in through one model of atelier's own (`crates/agents`): Claude Code, and atelier's own agent.

## Build and run

Rust 2024 (stable). macOS and Linux.

```sh
cargo run -p atelier-app            # the editor
cargo run -p beui-gallery         # every beui component in each state
tools/check.sh                    # every check a change needs: tests, clippy, the gallery, the vendored crates
```

An agent runs as its own program on the project's host, so install and sign in to the agent you want there
(for example `claude` for Claude Code).

## Layout

| Path | What it holds |
| --- | --- |
| `crates/app` | The window: projects, sessions, review, PR view, tasks, settings. |
| `crates/beui` | The design system, beui.dev on GPUI. See [docs/design-system.md](docs/design-system.md). |
| `crates/agents` | Agent sessions: the model, and a backend for each agent. See [docs/agents.md](docs/agents.md). |
| `crates/editor`, `crates/lsp` | The code editor and language servers. |
| `crates/project`, `crates/remote` | A project on this machine or on an SSH host. |
| `crates/review`, `crates/pr-view`, `crates/forge` | Review of agent turns, pull requests, and the code host. |
| `crates/tracker`, `crates/settings` | Tasks, and the settings file. |
| `crates/gallery` | The component gallery. |
| `vendor` | Patched copies of `gpui-base` and `gpui-component`. See each one's `PATCHES.md`. |
| `docs` | How each part works. [docs/vision.md](docs/vision.md) says where it goes. |

## License

[Apache-2.0](LICENSE). The copies in `vendor` keep their own Apache-2.0 license. Agent and lab logos in
`crates/agents/assets` belong to their owners and show which agent or model is in use.
