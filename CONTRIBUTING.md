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
- Depend on abstractions. Code that uses another module reaches it through that module's trait, never its
  struct: the app talks to `dyn Session`, not to `ClaudeSession`. A test hands in a fake that implements the
  same trait.
- Name a trait for the role and its implementation for what makes it particular: `Session` and
  `ClaudeSession`, `LineMapper` and `ClaudeLineMapper`. No `I` prefix and no `Impl` suffix. Struct names are
  singular.
- One item per file, in a folder for its kind. A module with more than one item becomes `<module>.rs` next to
  a `<module>/` folder (`lib.rs` at a crate root). The root file holds only its docs, its `mod` lines and its
  `pub use` lines. Inside the folder, each kind has a folder and a file that lists it:
  - `traits/`: one trait per file.
  - `structs/`: one struct per file, its fields only. Fields the module's `impls/` use are
    `pub(in super::super)`: seen by the module and no further. A function two `impls/` files share is
    `pub(super)`.
  - `enums/`: one enum per file.
  - `impls/`: the functions, one file per struct or enum, named the same, with every `impl` block of that
    type: its own and each `impl Trait for Type`. A type with many functions splits by topic, with the topic
    after the name: `mapper_turns.rs`, `mapper_tools.rs`.
  - `consts.rs`: the module's named constants and type aliases.
  - `tests/`: one file per struct, named the same, and the fakes the tests use. Tests only.
  A file is named after its item in snake_case: `BankAccount` lives in `bank_account.rs`. Create a folder only
  when it has something in it. The paths other code uses do not change: the root re-exports with `pub use`.
  A module still in the older one-file-per-kind layout moves to this one when a change touches it.
- Each function does one thing. A longer function reads as a list of calls to smaller functions whose names
  say what each step does. A helper only one type uses is a private function in that type's `impls/` file.
- One concept for each module. No inline `mod tests { ... }`.
- A bug fix comes with a test that fails without it.
