# Contributing to atelier

## Checks

Run `tools/check.sh` before you open a pull request. It runs the workspace tests, clippy with warnings as
errors, the gallery build, and the tests of the patched copies in `vendor`.

## Driving the app

A debug build listens on a Unix socket (`ATELIER_CONTROL=off` turns it off), so a change can be checked without a pointer:
`tools/dev-qa.sh start` runs a throwaway app on a scratch folder, `tools/atelier-ctl.sh state` lists the sessions and
the rows they show, `new_session [agent]` and `send "text"` act, and `tools/dev-qa.sh stop` ends it.

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
## Releasing (on a Mac)
The released app updates itself with [Sparkle](https://sparkle-project.org). An update downloads as a small patch
when one fits the version the reader has, else as the whole app. Only the Apple silicon Mac app is released.

Once, per Mac:
- A Developer ID Application certificate in the keychain; `ATELIER_SIGN_IDENTITY` names it.
- A notarytool profile: `xcrun notarytool store-credentials atelier-notary --apple-id <id> --team-id <team>`.
- The update signing key: `tools/mac/sparkle.sh` prints the folder of the pinned Sparkle, and
  `<folder>/bin/generate_keys --account dev.atelier.app` makes the key in the keychain. Export a copy to a safe place
  with `generate_keys --account dev.atelier.app -x <file>`: without the key, no later version can be signed, and the
  apps already installed refuse an update signed by another key.

Each release:
1. Raise `version` in the workspace `Cargo.toml`, and commit and push. A release is a commit; the script refuses a
   checkout with uncommitted changes, and an archive it has made is never rebuilt.
2. Build the Linux helper the app uploads (`tools/build-remote.sh` on a Linux x86_64 machine) and copy it here.
3. `tools/release-mac.sh build <atelier-remote-linux-x86_64>` signs the app with the hardened runtime, notarizes
   it, staples the ticket and archives it in `$ATELIER_RELEASES_DIR` (default `~/.local/share/atelier/releases`).
   Keep that folder: a patch is made from the archive of the version it updates.
4. `tools/release-mac.sh feed` makes `appcast.xml` and the patches, and checks every address, signature and length.
5. `tools/release-mac.sh publish` puts them on the `updates` release, the feed last, and creates the `vX.Y.Z` release.

To try an update first, without the real feed or app: `tools/mac/qa-update.sh prepare <helper>` builds two QA versions
and a feed with a patch between them, `serve` serves it, and the QA app (its own bundle id, settings and control socket)
installs it. The Check for Updates command is `{"cmd":"check_updates"}` on the control socket.
