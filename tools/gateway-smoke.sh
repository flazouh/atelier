#!/usr/bin/env bash
# Live smoke test of the agent gateway: a real `claude` lists the tasks of a gateway that holds two tasks in memory.
# It spends a few cents of model time, so it is not part of the test suite. Nothing real is read or written.
# Run it on the HP: tools/gateway-smoke.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

work="$(mktemp -d)"
cargo build -q -p atelier-gateway --example serve
target="${CARGO_TARGET_DIR:-target}"
"$target/debug/examples/serve" "$work/config-path" 120 &
server=$!
cleanup() { kill "$server" 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

for _ in $(seq 1 50); do [ -s "$work/config-path" ] && break; sleep 0.2; done
config="$(cat "$work/config-path")"
echo "gateway config: $config"

# Variables that start with CLAUDE_ or ANTHROPIC_ come from the session that runs this script, and would steer the
# `claude` of the test. It signs in as it does in a terminal.
strip=()
while IFS= read -r name; do strip+=(-u "$name"); done < <(env | sed -n 's/^\(CLAUDE_[^=]*\|ANTHROPIC_[^=]*\)=.*/\1/p')

env "${strip[@]}" claude --print --model haiku --mcp-config "$config" \
  --allowedTools mcp__atelier__tasks_list \
  --permission-mode default \
  "Use the tasks_list tool of the atelier server to list the tasks, then answer with one line per task: its title." \
  </dev/null
