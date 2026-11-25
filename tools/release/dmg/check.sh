#!/bin/bash
# Checks a disk image dmg.sh made, as a person's Mac will open it: mounted read-only, its files (the app, the
# Applications link, the picture, the volume's icon and its flag, the layout naming the picture), then its window as
# Finder lays it out, read back from Finder (bounds, icon view, toolbar, icon size, where each icon sits), against
# what dmg.sh set.
# Opens the window on this Mac's screen for a moment, and ejects the image after. Exits 1 on the first mismatch.
#   tools/release/dmg/check.sh RAEA.dmg [--keep-open]
set -euo pipefail
dmg=${1:?usage: check.sh RAEA.dmg [--keep-open]}
keep=; [ "${2:-}" = --keep-open ] && keep=1
fail() { echo "FAIL check: $*" >&2; exit 1; }
mnt=$(hdiutil attach -readonly -noverify -noautoopen "$dmg" 2>/dev/null | awk -F'\t' '/Apple_HFS/ {print $NF; exit}')
[ -n "$mnt" ] || fail "$dmg didn't mount"
[ -n "$keep" ] || trap 'osascript -e "tell application \"Finder\" to close (every window whose name is \"$vol\")" >/dev/null 2>&1 || true; hdiutil detach "$mnt" -quiet || hdiutil detach "$mnt" -force -quiet' EXIT
vol=$(basename "$mnt")
app=$(cd "$mnt" && ls -d -- *.app | head -1)
[ -n "$app" ] || fail "no app in $mnt"
[ "$(readlink "$mnt/Applications")" = /Applications ] || fail "Applications isn't a link to /Applications"
[ -f "$mnt/.background/background.tiff" ] || fail "no .background/background.tiff"
[ "$(tiffutil -info "$mnt/.background/background.tiff" | grep -c 'Image Width')" = 2 ] || fail "the picture isn't 1x and 2x"
[ -f "$mnt/.VolumeIcon.icns" ] || fail "no .VolumeIcon.icns"
GetFileInfo -aC "$mnt" | grep -qx 1 || fail "the volume's custom-icon flag is off: Finder shows a plain disk"
[ -f "$mnt/.DS_Store" ] || fail "no .DS_Store: the window has no layout"
# Finder won't hand the background picture to AppleScript (macOS 27), so it's read from the layout: its bookmark
strings -a "$mnt/.DS_Store" | grep -qx "/.background/background.tiff" || fail "the window's layout names no background.tiff"
codesign --verify --deep --strict "$mnt/$app" 2>/dev/null && signed="signed app" || signed="unsigned app"
layout=$(osascript - "$vol" "$app" <<'AS'
on run argv
	set volName to item 1 of argv
	set appName to item 2 of argv
	tell application "Finder"
		open disk volName
		set w to container window of disk volName
		set o to icon view options of w
		set b to bounds of w
		set p1 to position of item appName of w
		set p2 to position of item "Applications" of w
		return "view " & (current view of w as string) & "; toolbar " & (toolbar visible of w) & "; size " & ((item 3 of b) - (item 1 of b)) & "x" & ((item 4 of b) - (item 2 of b)) & "; icons " & (icon size of o) & "; app " & (item 1 of p1) & "," & (item 2 of p1) & "; Applications " & (item 1 of p2) & "," & (item 2 of p2)
	end tell
end run
AS
)
echo "window: $layout"
want="view icon view; toolbar false; size 680x468; icons 128; app 170,232; Applications 510,232"
[ "$layout" = "$want" ] || fail "the window isn't laid out as dmg.sh sets it: want \"$want\""
echo "ok $dmg: $vol, $app ($signed), the window as laid out"
