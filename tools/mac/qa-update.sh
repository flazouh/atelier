#!/usr/bin/env bash
# Tries an update end to end on this Mac, without touching the real app, the real feed or GitHub.
#
#   tools/mac/qa-update.sh prepare <atelier-remote-linux-x86_64>
#       builds two QA versions of the app (0.0.1 and 0.0.2, which differ by more than their numbers), makes a feed
#       with a patch from the first to the second, and installs the first in $ATELIER_QA_DIR/install.
#   tools/mac/qa-update.sh serve     serves that feed on http://127.0.0.1:8765 until `stop`
#   tools/mac/qa-update.sh stop      stops the server
#
# Then start $ATELIER_QA_DIR/install/atelier.app, ask it for updates (the control socket in $ATELIER_QA_DIR does:
# {"cmd":"check_updates"}), and look at what installs. The QA app keeps its settings and socket in $ATELIER_QA_DIR.
# Needs ATELIER_SIGN_IDENTITY, as tools/bundle-mac.sh does. The QA versions are signed but never notarized.
set -euo pipefail
cd "$(dirname "$0")/../.."
for dir in "$HOME/.cargo/bin" /opt/homebrew/bin /usr/local/bin; do
  if [ -d "$dir" ]; then PATH="$dir:$PATH"; fi
done
export PATH
qa="${ATELIER_QA_DIR:-$HOME/atelier-release/qa}"
port=8765
first=0.0.1
second=0.0.2

build_version() {
  version=$1
  ATELIER_RELEASE=1 ATELIER_QA=1 ATELIER_QA_DIR="$qa/state" ATELIER_BUNDLE_VERSION="$version" \
    ATELIER_FEED_URL="http://127.0.0.1:$port/appcast.xml" tools/bundle-mac.sh "$remote"
  mkdir -p "$qa/builds/$version"
  rm -rf "$qa/builds/$version/atelier.app"
  ditto target/bundle/atelier.app "$qa/builds/$version/atelier.app"
  ditto -c -k --sequesterRsrc --keepParent "$qa/builds/$version/atelier.app" "$qa/feed/atelier-$version-macos-arm64.zip"
}

prepare() {
  remote="${1:-}"
  [ -n "$remote" ] || { echo "Usage: tools/mac/qa-update.sh prepare <atelier-remote-linux-x86_64>" >&2; exit 1; }
  [ -n "${ATELIER_SIGN_IDENTITY:-}" ] || { echo "Set ATELIER_SIGN_IDENTITY." >&2; exit 1; }
  rm -rf "$qa/feed" "$qa/install" "$qa/state"
  mkdir -p "$qa/feed" "$qa/install" "$qa/state"
  build_version "$first"
  build_version "$second"
  sparkle=$(tools/mac/sparkle.sh)
  "$sparkle/bin/generate_appcast" --account "${ATELIER_SPARKLE_ACCOUNT:-dev.atelier.app}" \
    --download-url-prefix "http://127.0.0.1:$port/" "$qa/feed"
  deltas=$(grep -c '<enclosure' "$qa/feed/appcast.xml" || true)
  ls -la "$qa/feed"
  grep -q 'sparkle:deltas' "$qa/feed/appcast.xml" || { echo "The feed has no patch." >&2; exit 1; }
  ditto "$qa/builds/$first/atelier.app" "$qa/install/atelier.app"
  echo "Prepared: $first installed at $qa/install/atelier.app, $second in the feed ($deltas enclosures)."
}

serve() {
  [ -f "$qa/feed/appcast.xml" ] || { echo "Run prepare first." >&2; exit 1; }
  (cd "$qa/feed" && nohup python3 -m http.server "$port" --bind 127.0.0.1 > "$qa/server.log" 2>&1 & echo $! > "$qa/server.pid")
  sleep 1
  curl -fsS "http://127.0.0.1:$port/appcast.xml" >/dev/null && echo "Serving $qa/feed on http://127.0.0.1:$port"
}

stop() {
  if [ -f "$qa/server.pid" ]; then kill "$(cat "$qa/server.pid")" 2>/dev/null || true; rm -f "$qa/server.pid"; fi
}

case "${1:-}" in
  prepare) shift; prepare "$@" ;;
  serve) serve ;;
  stop) stop ;;
  *) echo "Usage: tools/mac/qa-update.sh prepare <atelier-remote-linux-x86_64> | serve | stop" >&2; exit 1 ;;
esac
