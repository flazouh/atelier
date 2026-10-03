#!/usr/bin/env bash
# Starts, shows and stops a throwaway atelier to check a change by driving it, not by guessing clicks.
#   tools/dev-qa.sh start [folder]   build, start on a scratch folder (a fresh git repo) with its own settings
#   tools/dev-qa.sh shot out.png     screenshot of the window
#   tools/dev-qa.sh stop
# Then drive it with tools/atelier-ctl.sh (state, new_session, send). With no display (the HP) it runs under
# Xvfb on :${QA_DISPLAY:-77}; with a display that answers, it opens a window there.
set -euo pipefail
cd "$(dirname "$0")/.."
RUN=${XDG_RUNTIME_DIR:-/tmp}/atelier-qa
mkdir -p "$RUN"
case "${1:-}" in
start)
  "$0" stop >/dev/null 2>&1 || true
  cargo build -q -p atelier-app
  TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  FOLDER=${2:-$RUN/project}
  # Only a folder that is not a repository yet becomes a scratch one; a checkout or a worktree is left as it is.
  if ! git -C "$FOLDER" rev-parse --git-dir >/dev/null 2>&1; then
    mkdir -p "$FOLDER"; git -C "$FOLDER" init -q
    echo "# scratch" >"$FOLDER/README.md"; git -C "$FOLDER" add -A
    git -C "$FOLDER" -c user.name=qa -c user.email=qa@qa -c commit.gpgsign=false commit -qm init
  fi
  if ! xdotool getdisplaygeometry >/dev/null 2>&1; then
    export DISPLAY=:${QA_DISPLAY:-77}
    Xvfb "$DISPLAY" -screen 0 1400x900x24 -nolisten tcp >"$RUN/xvfb.log" 2>&1 &
    echo $! >"$RUN/xvfb.pid"
    sleep 1
  fi
  echo "$DISPLAY" >"$RUN/display"
  rm -f "$RUN"/atelier-*.sock
  ATELIER_SETTINGS="$RUN/settings.json" ATELIER_CONTROL="$RUN/atelier.sock" WAYLAND_DISPLAY= \
    "$TARGET/debug/atelier" "$FOLDER" >"$RUN/app.log" 2>&1 &
  echo $! >"$RUN/app.pid"
  for _ in $(seq 60); do [ -S "$RUN/atelier.sock" ] && break; sleep 0.5; done
  [ -S "$RUN/atelier.sock" ] || { echo "the app did not start; see $RUN/app.log" >&2; exit 1; }
  # The socket answers before the folder has opened.
  for _ in $(seq 60); do ATELIER_CONTROL="$RUN/atelier.sock" "$(dirname "$0")/atelier-ctl.sh" state 2>/dev/null | grep -q '"name"' && break; sleep 0.5; done
  echo "export ATELIER_CONTROL=$RUN/atelier.sock DISPLAY=$(cat "$RUN/display")"
  ;;
shot)
  DISPLAY=$(cat "$RUN/display") import -window root "${2:?usage: dev-qa.sh shot out.png}"
  ;;
stop)
  for f in app xvfb; do
    [ -f "$RUN/$f.pid" ] && kill "$(cat "$RUN/$f.pid")" 2>/dev/null || true
    rm -f "$RUN/$f.pid"
  done
  rm -f "$RUN/atelier.sock"
  ;;
*)
  sed -n 2,7p "$0"; exit 2
  ;;
esac
