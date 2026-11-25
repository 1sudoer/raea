#!/bin/bash
# raea's Mac download as a disk image: raea.app beside a link to Applications in a Finder window laid out over
# raea's own picture (background.tiff, which raea renders from background.tsx: render.sh), with the app's icon as the
# volume's. A person opens it, drags the cat onto Applications, and raea runs from there (not from Downloads, where
# macOS runs a quarantined app translocated and read-only, so it can't update itself).
#
#   tools/release/dmg/dmg.sh APP.app OUT.dmg [--volname NAME] [--sign IDENTITY [--keychain KC]] [--notarize ENV]
#
# APP.app goes in as it is (ditto keeps its signature and stapled ticket). --sign signs the image with that Developer
# ID; --notarize ENV (a file setting ASC_KEY_PATH, ASC_KEY_ID and ASC_ISSUER_ID) notarizes and staples the signed image.
# Writes OUT.dmg and OUT.dmg.sha256. Finder writes the window's layout (.DS_Store) when layout.applescript asks it to,
# so this runs on a Mac in a logged-in session: the window opens and closes once while it's built.
# RAEA_DMG_FORMAT picks the compression (ULFO, lzfse, by default: macOS 10.11 and later, fast to verify and open).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
usage="usage: dmg.sh APP.app OUT.dmg [--volname NAME] [--sign IDENTITY [--keychain KC]] [--notarize ENV]"
app=${1:?$usage}
out=${2:?$usage}
vol="Install raea"
identity=
keychain=
notary=
shift 2
while [ $# -gt 0 ]; do
  case $1 in
    --volname) vol=${2:?$usage}; shift 2 ;;
    --sign) identity=${2:?$usage}; shift 2 ;;
    --keychain) keychain=${2:?$usage}; shift 2 ;;
    --notarize) notary=${2:?$usage}; shift 2 ;;
    *) echo "dmg: unknown option $1" >&2; exit 1 ;;
  esac
done
die() { echo "FAIL dmg: $*" >&2; exit 1; }
[ -d "$app/Contents" ] || die "$app isn't an app bundle"
[ -z "$notary" ] || [ -n "$identity" ] || die "--notarize needs --sign: Apple notarizes a signed image"
[ -f "$here/background.tiff" ] || die "no $here/background.tiff (render.sh makes it)"
[ ! -e "/Volumes/$vol" ] || die "a volume named \"$vol\" is mounted: eject it first, or Finder would lay out the wrong window"
format=${RAEA_DMG_FORMAT:-ULFO}

# the window, in points: the picture's size, and where Finder centres each icon on it (background.tsx's APP and APPS)
win_w=680; win_h=440
app_at=(170 232); apps_at=(510 232)
icon_px=128; text_px=13

name=$(basename "$app")
plist="$app/Contents/Info.plist"
icon=$(/usr/libexec/PlistBuddy -c "Print :CFBundleIconFile" "$plist" 2>/dev/null || true)
[ -n "$icon" ] || die "$plist names no CFBundleIconFile"
icon="$app/Contents/Resources/${icon%.icns}.icns"
[ -f "$icon" ] || die "no icon at $icon"

work=$(mktemp -d)
dev=
cleanup() {
  if [ -n "$dev" ]; then hdiutil detach "$dev" -quiet 2>/dev/null || hdiutil detach "$dev" -force -quiet 2>/dev/null || true; fi
  rm -rf "${work:?}"
}
trap cleanup EXIT

# a writable image a little larger than the app, attached where Finder sees it (no -nobrowse: AppleScript needs it)
mb=$(( $(du -sm "$app" | cut -f1) * 11 / 10 + 40 ))
rw="$work/rw.dmg"
hdiutil create -quiet -size "${mb}m" -fs HFS+ -volname "$vol" "$rw"
mnt=$(hdiutil attach -readwrite -noverify -noautoopen "$rw" 2>/dev/null | awk -F'\t' '/Apple_HFS/ {print $NF; exit}')
dev=$mnt # detached by its mount point
[ "$mnt" = "/Volumes/$vol" ] || die "attached at \"$mnt\", not /Volumes/$vol"

ditto "$app" "$mnt/$name"
ln -s /Applications "$mnt/Applications"
mkdir "$mnt/.background"
cp "$here/background.tiff" "$mnt/.background/background.tiff"

# its own line, so a failed layout stops the build: inside an echo's $(...), set -e never sees it fail, and the image
# would ship with no picture and Finder's default places
layout=$(osascript "$here/layout.applescript" "$vol" background.tiff "$win_w" "$win_h" "$name" \
  "${app_at[0]}" "${app_at[1]}" "${apps_at[0]}" "${apps_at[1]}" "$icon_px" "$text_px") ||
  die "Finder couldn't lay out the window (is this a logged-in session, and may this terminal control Finder?)"
echo "layout: $layout"
[ -f "$mnt/.DS_Store" ] || die "Finder wrote no .DS_Store: is this a logged-in session, and may this terminal control Finder?"

# the volume's icon after the layout: Finder (macOS 27) deletes a .VolumeIcon.icns, and clears the custom-icon flag,
# when it opens a volume that has them
cp "$icon" "$mnt/.VolumeIcon.icns"
SetFile -c icnC "$mnt/.VolumeIcon.icns"
SetFile -a C "$mnt"

rm -rf "${mnt:?}/.fseventsd" "${mnt:?}/.Trashes"
chmod -Rf go-w "$mnt" 2>/dev/null || true
sync
hdiutil detach "$dev" -quiet || hdiutil detach "$dev" -force -quiet
dev=

mkdir -p "$(dirname "$out")"
rm -f "$out"
hdiutil convert -quiet "$rw" -format "$format" -o "$out"

if [ -n "$identity" ]; then
  codesign --force --timestamp ${keychain:+--keychain "$keychain"} -s "$identity" "$out"
  codesign --verify --strict "$out"
fi
if [ -n "$notary" ]; then
  set -a; . "$notary"; set +a
  xcrun notarytool submit "$out" --key "$ASC_KEY_PATH" --key-id "$ASC_KEY_ID" --issuer "$ASC_ISSUER_ID" --wait \
    --output-format json | tee "$work/notary.json"
  grep -q '"status" *: *"Accepted"' "$work/notary.json" || die "notarization: $(cat "$work/notary.json")"
  xcrun stapler staple "$out"
  spctl --assess --type open --context context:primary-signature -vv "$out" 2>&1 | head -3
fi
# the file's own name, so `shasum -a 256 -c` works beside a download
(cd "$(dirname "$out")" && shasum -a 256 "$(basename "$out")") > "$out.sha256"
echo "ok $out ($(du -h "$out" | cut -f1), $format$([ -n "$identity" ] && echo ", signed")$([ -n "$notary" ] && echo ", notarized"))"
