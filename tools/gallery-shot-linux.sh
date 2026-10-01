#!/usr/bin/env bash
# Screenshots one gallery story on a headless Linux box (Xvfb + Mesa's software Vulkan). Nothing opens on screen.
# Build first: cargo build -p atelier-gallery.
# Usage: tools/gallery-shot-linux.sh "<story title>" light|dark out.png
# GALLERY_WAIT=<seconds> sets when the frame is taken; GALLERY_OPEN=1 opens menus on start.
set -euo pipefail
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
DISPLAY_NUM=:${GALLERY_DISPLAY:-99}
Xvfb "$DISPLAY_NUM" -screen 0 2200x1800x24 -nolisten tcp >/tmp/atelier-xvfb.log 2>&1 &
XVFB=$!
trap 'kill $GALLERY $XVFB 2>/dev/null || true' EXIT
sleep 1
DISPLAY=$DISPLAY_NUM WAYLAND_DISPLAY= GALLERY_THEME=$2 GALLERY_STORY=$1 "$TARGET/debug/atelier-gallery" >/tmp/atelier-gallery.log 2>&1 &
GALLERY=$!
sleep "${GALLERY_WAIT:-4}"
if ! kill -0 $GALLERY 2>/dev/null; then
  echo "gallery exited; see /tmp/atelier-gallery.log" >&2
  tail -20 /tmp/atelier-gallery.log >&2
  exit 1
fi
DISPLAY=$DISPLAY_NUM import -window root -trim +repage "$3"
