# Contributing to atelier

## Checks

Run `tools/check.sh` before you open a pull request. It runs the workspace tests, clippy with warnings as
errors, the gallery build, and the tests of the patched copies in `vendor`.

## Design

- The UI is built from [atelier-ui](https://github.com/flazouh/atelier-ui), a separate repo. A change to a
  component goes there first. To try it here before it is pushed, build against a local checkout next to this one:

  ```sh
  cargo run -p atelier-app --config 'patch."https://github.com/flazouh/atelier-ui".atelier-ui.path="../atelier-ui"'
  ```

  That build rewrites `Cargo.lock`; do not commit it. Once the change is in atelier-ui, run
  `cargo update -p atelier-ui` and commit the new lock.
- Every colour comes from the theme in force. Never write one in code: a test fails on a hex colour or an
  `rgb(` call.
- Every theme is borderless: surfaces part by tone and space, not by lines. A menu or dropdown panel that floats
  over the page is the one exception, with a subtle 1px border.
- Name an AI action in words. Never use a generic sparkle or magic-wand glyph for it.
- Icons are Material Symbols Rounded, copied with atelier-ui's `tools/material.sh`. Never draw one by hand.
- Components take plain data. atelier-ui knows nothing about Claude Code or any other agent: what is particular
  to an agent (its mark, its colour, its words) lives in `crates/agents` and reaches the UI as data.

## Motion

- Use the springs, curves and durations in atelier-ui's `motion.rs`. Add a named preset, not inline numbers.
- Every animation jumps to its end state when `cx.reduce_motion()` is true.

## Code

- A control belongs to the component it controls, in one place, and a component's layout is configured in one
  value. The sidebar's is `SidebarLayout`: the sidebar reads its rows and head from it, the Settings page edits
  it, and `crates/app/src/sidebar_layout.rs` alone maps it to the settings file. A new option is a new field of
  that value, never a constant or a flag elsewhere.
- Split a module by kind of item, one file per kind. A module with items of more than one kind becomes a
  folder of kind files: `<module>.rs` next to a `<module>/` folder (`lib.rs` at a crate root; a module that
  already has a `mod.rs` keeps it). The root file holds only its docs, its `mod` lines and its `pub use` lines,
  and the `#[cfg(test)] use` lines its `tests.rs` needs. Each kind has one file:
  - `structs.rs`: structs, with their `impl` blocks.
  - `types.rs`: enums and type aliases, with their `impl` blocks, and constants.
  - `traits.rs`: traits (the interfaces), and their blanket `impl` blocks.
  - `impls.rs`: `impl` blocks for a type that is not the module's own: a type of another crate or of another
    module, such as `impl From<Own> for io::Error`.
  - `helpers.rs`: free functions.
  - `tests.rs`, or `tests/<topic>.rs` when it grows: tests only.
  An `impl Trait for Type` stays with `Type` when the module defines `Type`. Create a file only when it has
  something in it. A module with one kind needs no split. The paths other code uses do not change: the root
  re-exports with `pub use`.
- One concept for each module: a kind file may hold several types of that one concept. No inline
  `mod tests { ... }`: put tests in `<module>/tests.rs` with `#[cfg(test)] mod tests;`.
- A bug fix comes with a test that fails without it.
