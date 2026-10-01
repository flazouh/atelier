#!/usr/bin/env bash
# Builds target/bundle/atelier.app. Run it on a Mac, from a checkout.
#
# Usage: tools/bundle-mac.sh <atelier-remote-linux-x86_64>
#
# <atelier-remote-linux-x86_64> is a release atelier-remote built for Linux x86_64. Build it on a Linux
# machine with `tools/build-remote.sh` (see docs/app.md, "Build the Mac app"). If a file named
# <that path>.sha256 sits beside it, the script checks the hash first.
#
# The script needs no network beyond what cargo needs to fetch crates. The icon comes from
# tools/mac/atelier-1024.png, so the Mac needs only the tools macOS ships (sips, iconutil, codesign).
# The app is signed ad hoc (`codesign -s -`), so it opens on the Mac that built it with no warning.
set -euo pipefail
cd "$(dirname "$0")/.."

[ "$(uname -s)" = Darwin ] || { echo "bundle-mac.sh runs on a Mac." >&2; exit 1; }
remote="${1:-}"
[ -n "$remote" ] && [ -f "$remote" ] || { echo "Usage: tools/bundle-mac.sh <atelier-remote-linux-x86_64>" >&2; exit 1; }

# The helper must be a Linux x86_64 build that speaks this protocol.
case "$(file -b "$remote")" in
  *ELF*x86-64*) ;;
  *) echo "$remote is not a Linux x86_64 executable." >&2; exit 1 ;;
esac
grep -aq "atelier-remote-protocol:" "$remote" || { echo "$remote has no protocol stamp; build it again." >&2; exit 1; }
if [ -f "$remote.sha256" ]; then
  want=$(awk '{print $1}' "$remote.sha256")
  have=$(shasum -a 256 "$remote" | awk '{print $1}')
  [ "$want" = "$have" ] || { echo "sha256 of $remote does not match $remote.sha256." >&2; exit 1; }
fi

for dir in "$HOME/.cargo/bin" /opt/homebrew/bin /usr/local/bin; do
  if [ -d "$dir" ]; then PATH="$dir:$PATH"; fi
done
export PATH

version=$(awk -F'"' '/^version *=/ {print $2; exit}' Cargo.toml)
target=aarch64-apple-darwin
host=$(rustc -vV | awk '/^host:/ {print $2}')
if [ "$host" = "$target" ]; then
  cargo build --release -p atelier-app
  profile=release
else
  cargo build --release -p atelier-app --target "$target"
  profile="$target/release"
fi
# Cargo may keep its target folder elsewhere (CARGO_TARGET_DIR, or a global config), so ask it.
built=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
exe="$built/$profile/atelier"
[ -f "$exe" ] || { echo "cargo built no $exe." >&2; exit 1; }

app=target/bundle/atelier.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/remote/linux-x86_64"
cp "$exe" "$app/Contents/MacOS/atelier"
cp "$remote" "$app/Contents/Resources/remote/linux-x86_64/atelier-remote"
chmod +x "$app/Contents/MacOS/atelier" "$app/Contents/Resources/remote/linux-x86_64/atelier-remote"

# The icon: the 1024 PNG at each size the icns wants.
icons=$(mktemp -d)/atelier.iconset
mkdir -p "$icons"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" tools/mac/atelier-1024.png --out "$icons/icon_${size}x${size}.png" >/dev/null
  sips -z "$((size * 2))" "$((size * 2))" tools/mac/atelier-1024.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Contents/Resources/atelier.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>atelier</string>
  <key>CFBundleDisplayName</key><string>atelier</string>
  <key>CFBundleIdentifier</key><string>dev.atelier.app</string>
  <key>CFBundleExecutable</key><string>atelier</string>
  <key>CFBundleIconFile</key><string>atelier</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

codesign --force --deep -s - "$app"
codesign --verify --deep --strict "$app"

echo "Built $app (version $version)."
echo "Run it: open $app"
echo "Move it: cp -R $app /Applications/"
