#!/bin/zsh
# Copies lucide icons (ISC, lucide-static), the set beui uses, into crates/beui/assets/icons.
# Usage: tools/lucide.sh <lucide-static>/icons <name>...   e.g. tools/lucide.sh /tmp/lucide/icons check copy
set -eu
SRC=$1; shift
for name in "$@"; do
  # Drop the license comment and the class attribute; GPUI reads the rest as is.
  sed -e '/<!--/d' -e '/class=/d' "$SRC/$name.svg" > "crates/beui/assets/icons/$name.svg"
done
