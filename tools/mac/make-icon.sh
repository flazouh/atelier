#!/usr/bin/env bash
# Draws tools/mac/atelier-1024.png, the icon for a Mac without Xcode (bundle-mac.sh uses
# tools/mac/atelier.icon when it has Xcode): the dark look of that icon, the "A" of
# crates/beui/assets/atelier-mark.svg in paper F0EEE6 on an ink 141413 rounded square, on the 1024 icon grid.
# Needs ImageMagick. bundle-mac.sh uses the PNG, so a Mac needs no ImageMagick. Run this only when the mark changes.
set -euo pipefail
cd "$(dirname "$0")/../.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sed 's/fill="#000"/fill="#F0EEE6"/' crates/beui/assets/atelier-mark.svg > "$work/a.svg"
convert -background none -density 1200 "$work/a.svg" -resize 630x "$work/a.png"
convert -size 1024x1024 xc:none -fill "#141413" -draw "roundrectangle 100,100 924,924 185,185" \
  "$work/a.png" -gravity center -composite tools/mac/atelier-1024.png
