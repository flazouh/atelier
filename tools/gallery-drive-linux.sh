#!/usr/bin/env bash
# Drives one gallery story on a headless Linux box and screenshots it: the screen is the window's size,
# so the window sits at 0,0 and the xdotool script's coordinates are the window's.
# Build first: cargo build -p beui-gallery.
# Usage: tools/gallery-drive-linux.sh "<story>" light|dark <width>x<height> out.png ["<xdotool script>"]
# GALLERY_WAIT=<seconds> sets how long the story runs before the script (a language server needs a while).
set -euo pipefail
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
DISPLAY_NUM=:${GALLERY_DISPLAY:-89}
Xvfb "$DISPLAY_NUM" -screen 0 "${3}x24" -nolisten tcp >/tmp/beui-xvfb.log 2>&1 &
XVFB=$!
trap 'kill $GALLERY $XVFB 2>/dev/null || true' EXIT
sleep 1
DISPLAY=$DISPLAY_NUM WAYLAND_DISPLAY= GALLERY_SIZE=$3 GALLERY_THEME=$2 GALLERY_STORY=$1 "$TARGET/debug/beui-gallery" >/tmp/beui-gallery.log 2>&1 &
GALLERY=$!
sleep "${GALLERY_WAIT:-8}"
export DISPLAY=$DISPLAY_NUM
if [ $# -ge 5 ]; then eval "$5"; fi
sleep 1
import -window root "$4"
