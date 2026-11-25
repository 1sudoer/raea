-- The installer window's layout (dmg.sh): Finder opens the mounted volume VOL, hides the toolbar and status bar,
-- sizes the window, sets icon view with the background picture, the icon and text size, and puts the app and the
-- Applications link where the background expects them. Finder keeps all of it in the volume's .DS_Store, which is
-- what a person's Finder reads when they open the disk image. Printed: the window's bounds and each item's position,
-- read back from Finder after it reopens the window.
--   osascript layout.applescript VOL BACKGROUND W H APP APP_X APP_Y APPS_X APPS_Y ICON_PX TEXT_PX
on run argv
	set volName to item 1 of argv
	set bgName to item 2 of argv
	set winW to (item 3 of argv) as integer
	set winH to (item 4 of argv) as integer
	set appName to item 5 of argv
	set appX to (item 6 of argv) as integer
	set appY to (item 7 of argv) as integer
	set lnX to (item 8 of argv) as integer
	set lnY to (item 9 of argv) as integer
	set iconPx to (item 10 of argv) as integer
	set textPx to (item 11 of argv) as integer
	-- a window's bounds include its title bar: 28 points to macOS 15, 32 on 26. The picture is drawn from the content's
	-- top left, so 28 shows all of it on the older ones and crops 4 points of plain paper on the newer.
	set titleBar to 28
	tell application "Finder"
		tell disk volName
			open
			set current view of container window to icon view
			set toolbar visible of container window to false
			set statusbar visible of container window to false
			set the bounds of container window to {160, 120, 160 + winW, 120 + winH + titleBar}
			set opts to the icon view options of container window
			set arrangement of opts to not arranged
			set icon size of opts to iconPx
			set text size of opts to textPx
			set shows item info of opts to false
			set shows icon preview of opts to false
			set background picture of opts to file (".background:" & bgName)
			set position of item appName of container window to {appX, appY}
			set position of item "Applications" of container window to {lnX, lnY}
			close
			open
			update without registering applications
			delay 1
			set b to bounds of container window
			set p1 to position of item appName of container window
			set p2 to position of item "Applications" of container window
			close
		end tell
	end tell
	return "bounds " & (item 1 of b) & "," & (item 2 of b) & "," & (item 3 of b) & "," & (item 4 of b) & "; " & appName & " " & (item 1 of p1) & "," & (item 2 of p1) & "; Applications " & (item 1 of p2) & "," & (item 2 of p2)
end run
