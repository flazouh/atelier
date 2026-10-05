#!/usr/bin/env bash
# Builds target/bundle/atelier.app. Run it on a Mac, from a checkout.
#
# Usage: tools/bundle-mac.sh <atelier-remote-linux-x86_64>
#
# <atelier-remote-linux-x86_64> is a release atelier-remote built for Linux x86_64. Build it on a Linux
# machine with `tools/build-remote.sh` (see docs/app.md, "Build the Mac app"). If a file named
# <that path>.sha256 sits beside it, the script checks the hash first.
#
# The script needs no network beyond what cargo needs to fetch crates. With Xcode, the icon is
# tools/mac/atelier.icon, which follows the light and dark appearance on macOS 26. Without it, the icon is
# tools/mac/atelier-1024.png, the dark look, so the Mac needs only the tools macOS ships (sips, iconutil, codesign).
# The app is signed ad hoc (`codesign -s -`), so it opens on the Mac that built it with no warning.
#
# A release build (ATELIER_RELEASE=1) is the app people install, and updates itself with Sparkle:
#   ATELIER_SIGN_IDENTITY  the Developer ID Application identity that signs it (required)
#   ATELIER_FEED_URL       the update feed; https, and the release's own unless ATELIER_QA=1 (default: the latest
#                          release's appcast.xml)
#   ATELIER_SPARKLE_ACCOUNT  the keychain account of the update signing key (default: dev.atelier.app)
#   ATELIER_BUNDLE_VERSION   the version to stamp, for a QA build (default: the workspace version)
# It embeds the pinned Sparkle (tools/mac/sparkle.sh), signs every part with the hardened runtime, and stops at the
# first thing missing. Notarize and publish with tools/release-mac.sh.
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
version="${ATELIER_BUNDLE_VERSION:-$version}"
release="${ATELIER_RELEASE:-0}"
update_keys=""
if [ "$release" = 1 ]; then
  [ -n "${ATELIER_SIGN_IDENTITY:-}" ] || { echo "A release build needs ATELIER_SIGN_IDENTITY (a Developer ID Application identity)." >&2; exit 1; }
  security find-identity -v -p codesigning | grep -qF "$ATELIER_SIGN_IDENTITY" || { echo "No signing identity matches $ATELIER_SIGN_IDENTITY." >&2; exit 1; }
  feed="${ATELIER_FEED_URL:-https://github.com/flazouh/atelier/releases/latest/download/appcast.xml}"
  case "$feed" in
    https://*) ;;
    http://*) [ "${ATELIER_QA:-0}" = 1 ] || { echo "The update feed must be https; $feed is not (ATELIER_QA=1 allows it for a QA build)." >&2; exit 1; } ;;
    *) echo "The update feed $feed is not a web address." >&2; exit 1 ;;
  esac
  sparkle=$(tools/mac/sparkle.sh)
  account="${ATELIER_SPARKLE_ACCOUNT:-dev.atelier.app}"
  public_key=$("$sparkle/bin/generate_keys" --account "$account" -p) || { echo "No update signing key for $account in the keychain." >&2; exit 1; }
  case "$public_key" in
    *ERROR*|"") echo "No update signing key for $account in the keychain." >&2; exit 1 ;;
  esac
  update_keys="  <key>SUFeedURL</key><string>${feed}</string>
  <key>SUPublicEDKey</key><string>${public_key}</string>
  <key>SUEnableAutomaticChecks</key><true/>
  <key>SUAutomaticallyUpdate</key><false/>
  <key>SUAllowsAutomaticUpdates</key><false/>
  <key>SUScheduledCheckInterval</key><integer>86400</integer>"
  case "$feed" in
    http://*) update_keys="$update_keys
  <key>NSAppTransportSecurity</key><dict><key>NSAllowsLocalNetworking</key><true/></dict>" ;;
  esac
fi
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
if [ "$release" = 1 ]; then
  # ditto keeps the framework's symbolic links, which a plain copy would follow and break.
  mkdir -p "$app/Contents/Frameworks"
  ditto "$sparkle/Sparkle.framework" "$app/Contents/Frameworks/Sparkle.framework"
fi

# The icon. actool compiles atelier.icon to Assets.car, with a light, a dark and a tinted look, and to
# atelier.icns for a macOS before 26. The "A" is atelier's mark, at 780 of the 1024 points.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
if xcrun --find actool >/dev/null 2>&1; then
  cp -R tools/mac/atelier.icon "$work/atelier.icon"
  mkdir -p "$work/atelier.icon/Assets"
  sed 's/<svg /<svg width="780" height="533" /' tools/mac/atelier-mark.svg > "$work/atelier.icon/Assets/mark.svg"
  xcrun actool "$work/atelier.icon" --compile "$app/Contents/Resources" --app-icon atelier \
    --enable-on-demand-resources NO --development-region en --target-device mac --platform macosx \
    --minimum-deployment-target 13.0 --output-partial-info-plist "$work/icon.plist" >/dev/null
else
  icons="$work/atelier.iconset"
  mkdir -p "$icons"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" tools/mac/atelier-1024.png --out "$icons/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" tools/mac/atelier-1024.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$icons" -o "$app/Contents/Resources/atelier.icns"
fi

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
  <key>CFBundleIconName</key><string>atelier</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSMicrophoneUsageDescription</key><string>Atelier listens to your microphone when you press the microphone button, to turn your speech into text on this Mac.</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
${update_keys}
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

if [ "$release" = 1 ]; then
  # Every part is signed on its own, innermost first, with the hardened runtime and a secure timestamp. `--deep` on
  # the outside would hide a part that failed to sign.
  framework="$app/Contents/Frameworks/Sparkle.framework/Versions/B"
  sign=(codesign --force --options runtime --timestamp --sign "$ATELIER_SIGN_IDENTITY")
  for part in "$framework/XPCServices/Installer.xpc" "$framework/XPCServices/Downloader.xpc" "$framework/Autoupdate" "$framework/Updater.app"; do
    [ -e "$part" ] && "${sign[@]}" "$part"
  done
  "${sign[@]}" "$app/Contents/Frameworks/Sparkle.framework"
  "${sign[@]}" --entitlements tools/mac/entitlements.plist "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
else
  codesign --force --deep -s - "$app"
  codesign --verify --deep --strict "$app"
fi

echo "Built $app (version $version)."
echo "Run it: open $app"
echo "Move it: cp -R $app /Applications/"
