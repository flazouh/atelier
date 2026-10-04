#!/usr/bin/env bash
# Draws tools/mac/atelier-1024.png, the icon for a Mac without Xcode (bundle-mac.sh uses tools/mac/atelier.icon when it
# has Xcode), on the 1024 icon grid: the white "A" of tools/mac/atelier-mark.svg on a rounded square of terracotta,
# lighter at the top, the AtelierMark of atelier-ui.
# Needs ImageMagick. bundle-mac.sh uses the PNG, so a Mac needs no ImageMagick. Run this only when the mark changes.
set -euo pipefail
cd "$(dirname "$0")/../.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tile="roundrectangle 100,100 924,924 185,185" # 824 wide; the corner is 0.23 of the side, as the AtelierMark's is
a=511                                          # the "A" is 0.62 of the tile, as the AtelierMark's is

sed 's/fill="#000"/fill="#FFFFFF"/' tools/mac/atelier-mark.svg > "$work/a.svg"
convert -background none -density 1200 "$work/a.svg" -resize "${a}x" "$work/a.png"
convert -size 1024x1024 xc:none -fill white -draw "$tile" "$work/shape.png"
# The accent E0875A, 18% toward white at the top and 12% toward black at the foot.
convert -size 1024x1024 gradient:"#E69D78-#C5764F" "$work/shape.png" -alpha off -compose CopyOpacity -composite \
  "$work/a.png" -gravity center -compose Over -depth 8 -composite tools/mac/atelier-1024.png
