#!/usr/bin/env bash
# Builds a release lathe-remote for this machine and puts it where the app looks for uploads:
# <dir>/<system>-<arch>/lathe-remote, <dir> being $LATHE_REMOTE_DIR or ~/.cache/lathe/remote-builds.
# Prints that folder, for LATHE_REMOTE_DIR on the machine that runs the app.
# Usage: tools/build-remote.sh    (a plain ssh shell has no cargo on PATH, so this sets it)
set -euo pipefail
cd "$(dirname "$0")/.."
for dir in "$HOME/.cargo/bin" "$HOME/.local/bin"; do
  if [ -d "$dir" ]; then PATH="$dir:$PATH"; fi
done
export PATH
cargo build -q --release -p lathe-remote
system=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$arch" in arm64) arch=aarch64 ;; amd64) arch=x86_64 ;; esac
out="${LATHE_REMOTE_DIR:-$HOME/.cache/lathe/remote-builds}"
mkdir -p "$out/$system-$arch"
cp target/release/lathe-remote "$out/$system-$arch/lathe-remote"
echo "$out"
