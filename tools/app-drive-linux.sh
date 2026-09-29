#!/usr/bin/env bash
# Drives the lathe app on a headless Linux box and screenshots or records it: the screen is the
# window's size, so the window sits at 0,0 and the xdotool script's coordinates are the window's.
# Build first: cargo build -p lathe-app.
# Usage: tools/app-drive-linux.sh "<folders, or ->" "<theme>" <width>x<height> out.png|out.mp4 ["<shell script>"]
# "-" opens no folder, for the start screen; several folders, split by spaces, open as several projects. The run keeps its own settings file, with that theme,
# unless LATHE_SETTINGS names one. APP_WAIT=<seconds> sets how long it runs before the script.
set -euo pipefail
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
DISPLAY_NUM=:${APP_DISPLAY:-79}
if [ -z "${LATHE_SETTINGS:-}" ]; then
  export LATHE_SETTINGS=$(mktemp -d)/settings.json
  printf '{"theme": "%s"}' "$2" >"$LATHE_SETTINGS"
fi
Xvfb "$DISPLAY_NUM" -screen 0 "${3}x24" -nolisten tcp >/tmp/lathe-xvfb.log 2>&1 &
XVFB=$!
trap 'kill $APP $XVFB 2>/dev/null || true' EXIT
sleep 1
FOLDER=()
if [ "$1" != "-" ]; then read -r -a FOLDER <<<"$1"; fi
DISPLAY=$DISPLAY_NUM WAYLAND_DISPLAY= LATHE_SIZE=$3 LATHE_TIMINGS=1 "$TARGET/debug/lathe" "${FOLDER[@]}" >/tmp/lathe-app.log 2>&1 &
APP=$!
sleep "${APP_WAIT:-6}"
export DISPLAY=$DISPLAY_NUM
if [[ "$4" == *.mp4 ]]; then
  ffmpeg -v error -y -f x11grab -framerate 30 -video_size "$3" -i "$DISPLAY_NUM" -pix_fmt yuv420p "$4" &
  REC=$!
  sleep 0.5
fi
if [ $# -ge 5 ]; then eval "$5"; fi
sleep 1
if [[ "$4" == *.mp4 ]]; then
  kill -INT $REC
  wait $REC || true
else
  import -window root "$4"
fi
