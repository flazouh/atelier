#!/usr/bin/env bash
# Releases atelier for the Mac, in three steps. Run it on a Mac, from a checkout of the commit being released.
#
#   tools/release-mac.sh build <atelier-remote-linux-x86_64>   sign, notarize and archive the app
#   tools/release-mac.sh feed                                  make the update feed and the patches from the archives
#   tools/release-mac.sh publish                               put the archives, patches and feed on GitHub
#
# Updates come from one GitHub release, `updates`, that holds every archive, every patch and appcast.xml, so every
# address in the feed is stable. Each version also gets its own release (`v0.1.0`) for people who download the app;
# it carries appcast.xml too, because the installed app asks the latest release for it.
#
# $ATELIER_RELEASES_DIR (default ~/.local/share/atelier/releases) keeps every archive ever released. Keep it: a patch
# is made from the archive of the version it updates, and an archive is never rebuilt, because the feed signs its bytes.
#
# Needs: ATELIER_SIGN_IDENTITY (a Developer ID Application identity) and a notarytool keychain profile named
# $ATELIER_NOTARY_PROFILE (default atelier-notary); see tools/bundle-mac.sh for the rest.
set -euo pipefail
cd "$(dirname "$0")/.."
[ "$(uname -s)" = Darwin ] || { echo "release-mac.sh runs on a Mac." >&2; exit 1; }
for dir in "$HOME/.cargo/bin" /opt/homebrew/bin /usr/local/bin; do
  if [ -d "$dir" ]; then PATH="$dir:$PATH"; fi
done
export PATH
repo="${ATELIER_REPO:-flazouh/atelier}"
channel=updates
releases="${ATELIER_RELEASES_DIR:-$HOME/.local/share/atelier/releases}"
profile="${ATELIER_NOTARY_PROFILE:-atelier-notary}"
account="${ATELIER_SPARKLE_ACCOUNT:-dev.atelier.app}"
version="${ATELIER_BUNDLE_VERSION:-$(awk -F'"' '/^version *=/ {print $2; exit}' Cargo.toml)}"
archive="$releases/atelier-$version-macos-arm64.zip"
app=target/bundle/atelier.app
base="https://github.com/$repo/releases/download/$channel/"

build() {
  remote="${1:-}"
  [ -n "$remote" ] || { echo "Usage: tools/release-mac.sh build <atelier-remote-linux-x86_64>" >&2; exit 1; }
  [ ! -e "$archive" ] || { echo "$archive exists. A released archive is never rebuilt; raise the version." >&2; exit 1; }
  commit=$(git rev-parse HEAD 2>/dev/null) || { echo "Release from a git checkout, so the release names its commit." >&2; exit 1; }
  [ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "The checkout has uncommitted changes; release a commit." >&2; exit 1; }
  # What people download must be source anyone can read: the commit is on the remote's main.
  git fetch -q origin main
  git merge-base --is-ancestor "$commit" origin/main || { echo "Commit $commit is not on origin/main; push it first." >&2; exit 1; }
  ATELIER_RELEASE=1 tools/bundle-mac.sh "$remote"
  notarize
  mkdir -p "$releases"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
  (cd "$releases" && shasum -a 256 "$(basename "$archive")" > "$(basename "$archive").sha256")
  echo "Archived $archive"
  printf '%s\n' "$commit" > "$archive.commit"
}

# Apple checks the app for malware; a stapled ticket lets it open with no network.
notarize() {
  zip=target/bundle/notarize.zip
  rm -f "$zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
  out=$(xcrun notarytool submit "$zip" --keychain-profile "$profile" --wait --output-format json)
  rm -f "$zip"
  status=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("status",""))')
  id=$(printf '%s' "$out" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id",""))')
  if [ "$status" != Accepted ]; then
    echo "Notarization ended as \"$status\" (submission $id):" >&2
    xcrun notarytool log "$id" --keychain-profile "$profile" >&2 || true
    exit 1
  fi
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=4 "$app"
}

# appcast.xml and the patches, made from the archives. generate_appcast signs each with the key in the keychain.
feed() {
  sparkle=$(tools/mac/sparkle.sh)
  [ -e "$archive" ] || { echo "No archive for $version in $releases; run build first." >&2; exit 1; }
  if [ -f "docs/release-notes/$version.md" ]; then cp "docs/release-notes/$version.md" "${archive%.zip}.md"; fi
  "$sparkle/bin/generate_appcast" --embed-release-notes --account "$account" --download-url-prefix "$base" "$releases"
  check_feed
}

# What the feed must be, said as code: every address is https and ours, every item is signed, every length is true.
check_feed() {
  python3 - "$releases" "$base" <<'PY'
import sys, os, re
import xml.etree.ElementTree as ET
folder, base = sys.argv[1], sys.argv[2]
ns = {"sparkle": "http://www.andymatuschak.org/xml-namespaces/sparkle"}
tree = ET.parse(os.path.join(folder, "appcast.xml"))
items = tree.getroot().findall("./channel/item")
assert items, "the feed has no item"
problems = []
for enclosure in tree.getroot().iter("enclosure"):
    url = enclosure.get("url", "")
    name = url.rsplit("/", 1)[-1]
    if not url.startswith(base):
        problems.append(f"{url} is not under {base}")
    if not enclosure.get("{%s}edSignature" % ns["sparkle"]):
        problems.append(f"{name} has no edSignature")
    path = os.path.join(folder, name)
    if not os.path.exists(path):
        problems.append(f"{name} is in the feed but not in {folder}")
    elif str(os.path.getsize(path)) != enclosure.get("length"):
        problems.append(f"{name} is {os.path.getsize(path)} bytes, the feed says {enclosure.get('length')}")
if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
versions = [i.find("sparkle:version", ns).text for i in items]
deltas = sum(len(i.findall("./sparkle:deltas/enclosure", ns)) for i in items)
print(f"feed ok: versions {versions}, {deltas} patch(es)")
PY
}

# The archives, patches and feed go to the update channel, the feed last, so a reader of the feed finds every file it
# names. The version's own release is a draft until its archive is checked, then it is published.
publish() {
  check_feed
  if ! gh release view "$channel" --repo "$repo" >/dev/null 2>&1; then
    gh release create "$channel" --repo "$repo" --prerelease --title "Update channel" \
      --notes "Files the app downloads to update itself. Download the app from the version releases."
  fi
  existing=$(gh release view "$channel" --repo "$repo" --json assets --jq '.assets[].name')
  for file in "$releases"/*.zip "$releases"/*.delta; do
    [ -e "$file" ] || continue
    if ! printf '%s\n' "$existing" | grep -qxF "$(basename "$file")"; then
      gh release upload "$channel" "$file" --repo "$repo"
    fi
  done
  gh release upload "$channel" "$releases/appcast.xml" --repo "$repo" --clobber
  tag="v$version"
  if ! gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    notes="docs/release-notes/$version.md"
    [ -f "$notes" ] || { printf 'atelier %s for Apple silicon Macs.\n' "$version" > target/release-notes.md; notes=target/release-notes.md; }
    # The app asks the latest release for appcast.xml (SUFeedURL), so each version's release carries the feed as well.
    gh release create "$tag" --repo "$repo" --draft --title "atelier $version" --notes-file "$notes" "$archive" "$archive.sha256" "$releases/appcast.xml" --target "$(cat "$archive.commit")"
    check_published "$tag"
    gh release edit "$tag" --repo "$repo" --draft=false
  else
    gh release upload "$tag" "$releases/appcast.xml" --repo "$repo" --clobber
  fi
  check_latest_feed
  echo "Published $tag and the update channel."
}

# The address the installed app asks, as anyone would ask it. GitHub can take several minutes to serve a new file.
check_latest_feed() {
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' RETURN
  for _ in $(seq 1 60); do
    if curl -fsSL "https://github.com/$repo/releases/latest/download/appcast.xml" -o "$tmp/appcast.xml" 2>/dev/null; then
      cmp -s "$tmp/appcast.xml" "$releases/appcast.xml" && { echo "The app's feed address serves the feed made."; return 0; }
      echo "The app's feed address serves a different feed." >&2; exit 1
    fi
    sleep 10
  done
  echo "The app's feed address (releases/latest/download/appcast.xml) serves nothing." >&2
  exit 1
}

# The files, fetched as anyone would fetch them: no login, the real addresses.
check_published() {
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' RETURN
  curl -fsSL "${base}appcast.xml" -o "$tmp/appcast.xml"
  curl -fsSL "${base}$(basename "$archive")" -o "$tmp/archive.zip"
  want=$(awk '{print $1}' "$archive.sha256")
  have=$(shasum -a 256 "$tmp/archive.zip" | awk '{print $1}')
  [ "$want" = "$have" ] || { echo "The published archive differs from the one built." >&2; exit 1; }
  cmp -s "$tmp/appcast.xml" "$releases/appcast.xml" || { echo "The published feed differs from the one made." >&2; exit 1; }
  echo "The published feed and archive match what was built."
}

case "${1:-}" in
  build) shift; build "$@" ;;
  feed) feed ;;
  publish) publish ;;
  *) echo "Usage: tools/release-mac.sh build <atelier-remote-linux-x86_64> | feed | publish" >&2; exit 1 ;;
esac
