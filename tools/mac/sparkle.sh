#!/usr/bin/env bash
# Prints the folder of the pinned Sparkle release (Sparkle.framework and its bin/ tools), fetching it first when it
# is not there. The release is pinned by version and by the sha256 of its archive: the build never takes a newer one.
# Usage: tools/mac/sparkle.sh    (a Mac)
set -euo pipefail
version=2.10.0
sha256=c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c
dir="${ATELIER_SPARKLE_DIR:-$HOME/.cache/atelier/sparkle}/$version"
if [ ! -d "$dir/Sparkle.framework" ]; then
  mkdir -p "$dir"
  archive="$dir/Sparkle-$version.tar.xz"
  curl -fsSL "https://github.com/sparkle-project/Sparkle/releases/download/$version/Sparkle-$version.tar.xz" -o "$archive"
  have=$(shasum -a 256 "$archive" | awk '{print $1}')
  if [ "$have" != "$sha256" ]; then
    rm -f "$archive"
    echo "Sparkle $version has sha256 $have, not the pinned $sha256." >&2
    exit 1
  fi
  tar -xf "$archive" -C "$dir"
fi
echo "$dir"
