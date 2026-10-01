#!/usr/bin/env bash
# Every check a change needs before review: the workspace tests (with a live rust-analyzer), clippy,
# the gallery build, and the tests of the patched gpui-base and gpui-component copies, which the
# workspace excludes.
# Usage: tools/check.sh    (run on hp-agent, where rust-analyzer is installed)
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=C.UTF-8
# A plain `ssh hp-agent tools/check.sh` gets no login PATH: cargo, and the go that gopls needs, live here.
for dir in "$HOME/.cargo/bin" "$HOME/.local/bin"; do
  if [ -d "$dir" ]; then PATH="$dir:$PATH"; fi
done
export PATH
# The tests make their temporary folders in a folder of this run, not in the shared /tmp, and must remove them:
# a run that leaves any behind fails. (A leak once filled the inodes of /tmp on hp-agent.) The folder is outside
# the checkout, because a test that expects "no git repository" would find this one above it. The gallery keeps
# fixtures under fixed names on purpose, and the claude program and rust-analyzer keep their own folders; none of these count.
run_tmp=$(mktemp -d /tmp/atelier-check.XXXXXX)
trap 'rm -rf "$run_tmp"' EXIT
export TMPDIR="$run_tmp"
ATELIER_REQUIRE_LSP=rust,typescript,python,go cargo test -q --workspace
left=$(find "$run_tmp" -mindepth 1 -maxdepth 1 ! -name 'atelier-gallery-*' ! -name 'claude-*' ! -name 'proc-macro-srv*' | wc -l)
if [ "$left" -ne 0 ]; then
  echo "the tests left $left temporary folders in $run_tmp:" >&2
  find "$run_tmp" -mindepth 1 -maxdepth 1 ! -name 'atelier-gallery-*' ! -name 'claude-*' ! -name 'proc-macro-srv*' | head -10 >&2
  exit 1
fi
unset TMPDIR
cargo clippy -q --workspace --all-targets -- -D warnings
cargo build -q -p beui-gallery
# vendor/gpui-base is excluded from the workspace, so its tests, the patch tests among them, only
# run here. Its own target directory keeps its lock file from touching ours.
CARGO_TARGET_DIR="$PWD/target/vendor" cargo test -q --manifest-path vendor/gpui-base/Cargo.toml --lib
# gpui-component's own unit tests read files from its repository that the crate does not ship, so
# only its patch test runs here.
CARGO_TARGET_DIR="$PWD/target/vendor-component" cargo test -q --manifest-path vendor/gpui-component/Cargo.toml \
  --features tree-sitter-languages --test injection_edits --test background_parse
echo "all checks passed"
