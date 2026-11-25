# Sourced by tools/skills.sh (its feed app, split out to keep it under 500 lines): where the feed app's files are read
# from, as APP and APP_REV, and app_show and app_grep to read them. The app is the one in this tree when the tree has it
# (apps/app is raea's own since 11-16, docs/public-first.feature), read as its files are, so what's generated follows the
# commit being checked. A tree without it (before 11-16) reads RAEA_APP, the app's own checkout (default apps/app; L0
# names the shared one), at RAEA_APP_REV, the commit raea's tree records for it (L0 passes its tree's), not whatever that
# checkout has on disk. check-cli reads none of it.
# shellcheck shell=bash
APP=${RAEA_APP:-apps/app} APP_REV=
# check-cli reads only raea's help, so it runs where the feed app isn't (a public checkout: release.yml's runners)
if [ "${1:-}" != check-cli ]; then
  # the app in this tree, tracked here: its files as they are, whatever RAEA_APP names
  if git ls-files --error-unmatch apps/app/src/acp/protocol.ts >/dev/null 2>&1; then APP=apps/app; fi
  [ -f "$APP/src/acp/protocol.ts" ] || { echo "skills.sh: no feed app at $APP (RAEA_APP names its checkout)" >&2; exit 2; }
  APP=$(cd "$APP" && pwd)
  # an app with a checkout of its own (raea-feed, before 11-16): read at the commit raea records
  if [ -e "$APP/.git" ]; then
    APP_REV=${RAEA_APP_REV:-$(git rev-parse -q --verify HEAD:apps/app 2>/dev/null || git -C "$APP" rev-parse HEAD)}
    git -C "$APP" cat-file -e "$APP_REV" 2>/dev/null || { echo "skills.sh: apps/app has no commit $APP_REV (fetch raea-feed)" >&2; exit 2; }
  fi
fi
# a file of the app's, from its root: at APP_REV in its own checkout, else as it is in this tree
app_show() { if [ -n "$APP_REV" ]; then git -C "$APP" show "$APP_REV:$1"; else cat "$APP/$1"; fi; }
# git grep over the app's tracked files (OPTION... PATTERN -- PATHSPEC...), paths from its root
app_grep() {
  local a=()
  while [ $# -gt 0 ] && [ "$1" != -- ]; do a+=("$1"); shift; done
  if [ -n "$APP_REV" ]; then git -C "$APP" grep "${a[@]}" "$APP_REV" "$@" | sed "s|^$APP_REV:||"
  else git -C "$APP" grep "${a[@]}" "$@"; fi
}
