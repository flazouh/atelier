# The web: a website and a web app

Status: plan, 2026-09-30. Nothing is built. It fixes the rules so that today's code stays reusable.

## What reuses what

| Part | Built with | Reuses from the desktop app |
|---|---|---|
| The Hub (M8), a server | Rust, for example `axum` | Everything that is not UI: tracker, forge, agents, review, remote protocol, settings types |
| The web app, in the browser | A Rust web framework (Leptos or Dioxus), compiled to WebAssembly | The pure crates: the review and diff model, task rules, the data types, the protocol types |
| The web app's look | CSS made from our theme data | The theme tokens (colours, sizes, radii, type), exported as CSS variables by one crate |
| The website | A static site tool | Nothing from Rust; it takes the logo, the tokens and the screenshots |

The desktop UI does not move to the web. GPUI draws on the desktop only and has no web target, so the web
app has its own components. It shares the logic and the look, not the widget code.

## What runs where

- In the browser: pure logic only. No process, no file system, no SSH.
- On the Hub: everything that spawns `git`, opens SQLite, reaches GitHub or runs an agent. The web app asks
  the Hub for it.

## Rules for the code, from now on

1. A crate with logic or data has no UI dependency: no `gpui`, no `beui`. UI crates depend on logic crates,
   never the other way.
2. A crate the browser needs builds for `wasm32-unknown-unknown`. It keeps processes, files and the network
   behind a trait that the desktop and the Hub implement.
3. Data types derive `serde`, so that the Hub and the web app can send them.
4. Themes stay data. A new token goes in the theme data, not in a component.

## First step (queued)

- Move the types that `crates/review` takes from `beui` (such as `InlineHunk`) into a UI-free crate, so that
  `crates/review` no longer depends on `beui`.
- A CI check: the pure crates build for `wasm32-unknown-unknown`, and none depends on `gpui` or `beui`.
