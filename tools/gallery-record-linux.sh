#!/usr/bin/env bash
# Records a gallery story on a headless Linux box, so motion can be checked frame by frame.
# Build first: cargo build -p atelier-gallery.
# Usage: tools/gallery-record-linux.sh "<story>" light|dark <seconds> out.mp4 ["<xdotool script>"]
# The xdotool script runs after the window opens, with $C and $R set to the story area's top left.
set -euo pipefail
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
DISPLAY_NUM=:${GALLERY_DISPLAY:-79}
Xvfb "$DISPLAY_NUM" -screen 0 1400x1000x24 -nolisten tcp >/tmp/atelier-xvfb.log 2>&1 &
XVFB=$!
trap 'kill $GALLERY $XVFB 2>/dev/null || true' EXIT
sleep 1
DISPLAY=$DISPLAY_NUM GALLERY_THEME=$2 GALLERY_STORY=$1 "$TARGET/debug/atelier-gallery" >/tmp/atelier-gallery.log 2>&1 &
GALLERY=$!
sleep "${GALLERY_WAIT:-5}"
export C=$((152 + 220)) R=$((70 + 90)) DISPLAY=$DISPLAY_NUM
ffmpeg -v error -y -f x11grab -framerate 60 -video_size 1400x1000 -i "$DISPLAY_NUM" -t "$3" -pix_fmt yuv420p "$4" &
REC=$!
sleep 0.5
if [ $# -ge 5 ]; then eval "$5"; fi
wait $REC
