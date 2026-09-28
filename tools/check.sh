#!/usr/bin/env bash
# Every check a change needs before review: the workspace tests (with a live rust-analyzer), clippy,
# the gallery build, and the tests of the patched gpui-base and gpui-component copies, which the
# workspace excludes.
# Usage: tools/check.sh    (run on hp-agent, where rust-analyzer is installed)
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=C.UTF-8
LATHE_REQUIRE_LSP=rust,typescript,python,go cargo test -q --workspace
cargo clippy -q --workspace --all-targets -- -D warnings
cargo build -q -p beui-gallery
# vendor/gpui-base is excluded from the workspace, so its tests, the patch tests among them, only
# run here. Its own target directory keeps its lock file from touching ours.
CARGO_TARGET_DIR="$PWD/target/vendor" cargo test -q --manifest-path vendor/gpui-base/Cargo.toml --lib
# gpui-component's own unit tests read files from its repository that the crate does not ship, so
# only its patch test runs here.
CARGO_TARGET_DIR="$PWD/target/vendor-component" cargo test -q --manifest-path vendor/gpui-component/Cargo.toml \
  --features tree-sitter-languages --test injection_edits
echo "all checks passed"
