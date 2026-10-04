#!/usr/bin/env bash
# Draws the icon for a Mac without Xcode (bundle-mac.sh uses tools/mac/atelier.icon when it has Xcode), twice, on the
# 1024 icon grid, with the "A" of tools/mac/atelier-mark.svg, the shape of the AtelierMark in atelier-ui:
#   tools/mac/atelier-1024.png            the Halo: a paper F0EEE6 "A" on an ink 141413 rounded square, a thin light edge
#                                         and a glow of the accent E0875A under the "A". The icon bundle-mac.sh uses.
#   tools/mac/atelier-terracotta-1024.png the Terracotta: an ink "A" on a rounded square of the accent, lighter at the top.
# Needs ImageMagick. bundle-mac.sh uses the PNG, so a Mac needs no ImageMagick. Run this only when the mark changes.
set -euo pipefail
cd "$(dirname "$0")/../.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tile="roundrectangle 100,100 924,924 185,185" # 824 wide; the corner is 0.23 of the side, as the AtelierMark's is
a=511                                          # the "A" is 0.62 of the tile, as the AtelierMark's is

draw_a() { # colour, file
  sed "s/fill=\"#000\"/fill=\"$1\"/" tools/mac/atelier-mark.svg > "$work/a.svg"
  convert -background none -density 1200 "$work/a.svg" -resize "${a}x" "$2"
}

convert -size 1024x1024 xc:none -fill white -draw "$tile" "$work/shape.png"

# Halo: ink tile, glow rising from the foot (a blurred ellipse kept inside the tile), edge, paper "A".
draw_a "#F0EEE6" "$work/paper-a.png"
convert -size 1024x1024 xc:none -fill "#E0875A" -draw "ellipse 512,900 330,110 0,360" -blur 0x55 \
  "$work/shape.png" -compose DstIn -composite -channel A -evaluate multiply 0.55 +channel "$work/glow.png"
convert -size 1024x1024 xc:none -fill none -stroke "rgba(240,238,230,0.14)" -strokewidth 3 \
  -draw "roundrectangle 101.5,101.5 922.5,922.5 183.5,183.5" "$work/edge.png"
convert -size 1024x1024 xc:none -fill "#141413" -draw "$tile" "$work/glow.png" -composite "$work/edge.png" -composite \
  "$work/paper-a.png" -gravity center -depth 8 -composite tools/mac/atelier-1024.png

# Terracotta: the accent from a lighter top to a darker foot (+18% white, +12% black), ink "A".
draw_a "#141413" "$work/ink-a.png"
convert -size 1024x1024 gradient:"#E69D78-#C5764F" "$work/ramp.png"
convert "$work/ramp.png" "$work/shape.png" -alpha off -compose CopyOpacity -composite \
  "$work/ink-a.png" -gravity center -compose Over -depth 8 -composite tools/mac/atelier-terracotta-1024.png
