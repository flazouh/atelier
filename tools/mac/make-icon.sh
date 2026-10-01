#!/usr/bin/env bash
# Draws tools/mac/atelier-1024.png: beui's AtelierMark (crates/beui/src/atelier_mark.rs) on the 1024 icon grid.
# Ink 141413 disc, the "A" of crates/beui/assets/atelier-mark.svg in paper F0EEE6 at 0.6 of the disc's width.
# Needs ImageMagick. bundle-mac.sh uses the PNG, so a Mac needs no ImageMagick. Run this only when the mark changes.
set -euo pipefail
cd "$(dirname "$0")/../.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sed 's/fill="#000"/fill="#F0EEE6"/' crates/beui/assets/atelier-mark.svg > "$work/a.svg"
convert -background none -density 1200 "$work/a.svg" -resize 494x "$work/a.png"
convert -size 1024x1024 xc:none -fill "#141413" -draw "circle 512,512 512,100" \
  "$work/a.png" -gravity center -composite tools/mac/atelier-1024.png
