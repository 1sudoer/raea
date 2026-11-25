#!/bin/bash
# A raea project's repo on raea's servers (docs/cloud-render.feature CR21, CR22): its Git on Artifacts, its media
# through raea's LFS into R2, signed in by git-credential-raea beside this script. The same on a Mac, Linux, a cloud
# box and Windows' Git Bash. RAEA_API_ORIGIN, RAEA_AGENT_KEY / RAEA_TOKEN as git-credential-raea says.
#
#   repo.sh create DIR PROJECT     DIR (a folder, a git repo or not) becomes PROJECT's repo: made on raea's server if it's
#                                  new, .lfsconfig pointing at raea, media decided by type and size (MEDIA, LARGE as
#                                  the app's host/repo.ts), committed and pushed
#   repo.sh clone OWNER PROJECT DIR   the project's Git, then its media (git lfs pull)
#   repo.sh sign DIR OWNER PROJECT    a clone made another way signed in like the above
set -euo pipefail
# beside this script itself, run through a link or not (the box runs it as /usr/local/bin/raea-repo)
self=$0
[ -L "$self" ] && self=$(readlink -f "$self" 2>/dev/null || readlink "$self")
here=$(cd "$(dirname "$self")" && pwd)
api=${RAEA_API_ORIGIN:-https://raea.io}
# git runs a helper named by its absolute path directly, so a clone keeps working after this script is done
helper=$here/git-credential-raea

call() {
  local t=${RAEA_AGENT_KEY:-${RAEA_TOKEN:-}}
  [ -n "$t" ] || t=$(/usr/bin/security find-generic-password -s "${RAEA_KEYCHAIN:-raea}" -a device-token -w 2>/dev/null) || { echo "repo: no raea sign-in" >&2; exit 1; }
  # a refusal says why (402 enterprise_required: Git on raea's servers is raea Enterprise's for now)
  local out
  out=$(curl -sS --fail-with-body -m 60 -X "$1" -H "authorization: Bearer $t" -H 'content-type: application/json' ${3:+-d "$3"} "$api$2") || {
    echo "repo: raea said no to $1 $2: ${out:-no answer}" >&2
    exit 1
  }
  printf %s "$out"
}
field() { python3 -c 'import json, sys; print(json.load(sys.stdin)[sys.argv[1]])' "$1"; }
hostof() { local u=${1#*://}; printf '%s://%s' "${1%%://*}" "${u%%/*}"; }

# git's config for raea in a repo: the helper for both hosts, which project it is, LFS with its locks
sign() {
  local dir=$1 owner=$2 project=$3 remote lfs
  remote=$(git -C "$dir" remote get-url origin)
  lfs=$(git -C "$dir" config -f "$dir/.lfsconfig" --get lfs.url)
  git -C "$dir" config raea.owner "$owner"
  git -C "$dir" config raea.project "$project"
  for h in "$(hostof "$remote")" "$(hostof "$lfs")"; do
    git -C "$dir" config "credential.$h.helper" ""
    git -C "$dir" config --add "credential.$h.helper" "$helper"
  done
  git -C "$dir" lfs install --local >/dev/null
  git -C "$dir" config lfs.locksverify true
}

# media by type, whatever the size, and anything at or over LARGE (host/repo.ts's MEDIA and LARGE)
MEDIA='mp4 mov m4v webm mkv avi mxf mp3 wav aiff aif flac m4a ogg opus caf png jpg jpeg heic heif webp gif tif tiff exr hdr psd blend glb gltf fbx obj usd usdz abc ply splat spz ktx2 ttf otf woff woff2 zip tar gz zst safetensors onnx bin npz npy pdf'
LARGE=$((4 * 1024 * 1024))
track() {
  local dir=$1 f ext
  for ext in $MEDIA; do git -C "$dir" lfs track "*.$ext" >/dev/null; done
  # a big file of another type goes to LFS by its own path
  while IFS= read -r f; do
    git -C "$dir" check-attr filter -- "$f" | grep -q ': lfs$' || git -C "$dir" lfs track --filename "$f" >/dev/null
  done < <(cd "$dir" && find . -type f -size +$((LARGE / 1024 - 1))k -not -path './.git/*' | sed 's|^\./||')
}

case "${1:-}" in
create)
  dir=$2 project=$3
  made=$(call POST /v1/repos "{\"project\":\"$project\"}")
  owner=$(printf %s "$made" | field owner); remote=$(printf %s "$made" | field remote); lfs=$(printf %s "$made" | field lfs)
  [ -d "$dir/.git" ] || git -C "$dir" init -q -b main
  git -C "$dir" remote get-url origin >/dev/null 2>&1 && git -C "$dir" remote set-url origin "$remote" || git -C "$dir" remote add origin "$remote"
  git -C "$dir" config -f "$dir/.lfsconfig" lfs.url "$lfs"
  sign "$dir" "$owner" "$project"
  track "$dir"
  git -C "$dir" add -A
  git -C "$dir" diff --cached --quiet || git -C "$dir" commit -qm "${RAEA_REPO_MESSAGE:-raea: the project with raea}"
  git -C "$dir" push -q -u origin HEAD:main
  echo "repo: $project at $remote, media through $lfs"
  ;;
clone)
  owner=$2 project=$3 dir=$4
  remote=$(call GET /v1/repos | python3 -c 'import json, sys; o, p = sys.argv[1:]; print(next((r["remote"] for r in json.load(sys.stdin)["repos"] if r["owner"] == o and r["project"] == p), ""))' "$owner" "$project")
  [ -n "$remote" ] || { echo "repo: no project $project for $owner here" >&2; exit 1; }
  h=$(hostof "$remote")
  GIT_LFS_SKIP_SMUDGE=1 git -c raea.owner="$owner" -c raea.project="$project" -c "credential.$h.helper=" -c "credential.$h.helper=$helper" clone -q "$remote" "$dir"
  sign "$dir" "$owner" "$project"
  git -C "$dir" lfs pull
  echo "repo: cloned $project into $dir"
  ;;
sign) sign "$2" "$3" "$4" ;;
*) sed -n '2,12p' "$0"; exit 2 ;;
esac
