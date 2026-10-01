#!/usr/bin/env bash
# Draws tools/mac/atelier-1024.png: beui's AtelierMark (crates/beui/src/atelier_mark.rs) on the 1024 icon grid.
# Ink 141413 circle, paper F0EEE6 letter, Geist Medium. Needs ImageMagick.
# bundle-mac.sh uses the PNG, so a Mac needs no ImageMagick. Run this only when the mark changes.
set -euo pipefail
cd "$(dirname "$0")/../.."
convert -size 1024x1024 xc:none -fill "#141413" -draw "circle 512,512 512,100" \
  -font crates/beui/assets/fonts/Geist-Medium.ttf -pointsize 560 -fill "#F0EEE6" \
  -gravity center -annotate +0+0 "l" tools/mac/atelier-1024.png
