#!/bin/zsh
# Screenshots one gallery story. Build first: cargo build -p beui-gallery.
# Usage: tools/gallery-shot.sh "<story title>" light|dark out.png
set -eu
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
GALLERY_THEME=$2 GALLERY_STORY=$1 "$TARGET/debug/beui-gallery" >/tmp/beui-gallery.log 2>&1 &
PID=$!
sleep 3
WID=$(swift -e "import CoreGraphics; let l = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as! [[String: Any]]; for w in l { if (w[\"kCGWindowOwnerPID\"] as? Int) == $PID { print(w[\"kCGWindowNumber\"]!); break } }")
if [ -n "$WID" ]; then screencapture -x -o -l"$WID" "$3"; else echo "no gallery window for pid $PID" >&2; fi
kill $PID 2>/dev/null || true
