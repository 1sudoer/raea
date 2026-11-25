# Sourced by raea's Modal scripts (workspace.sh, models.sh): MODAL_PROFILE must name a profile whose token reaches the
# workspace of the same name (raea's is raea; sdk/cloud/deploy/login.sh makes the profile), and never a workspace
# listed in private/cloud/modal-denied.txt (workspaces that belong to other businesses). The default profile is never
# used and never changed. (No ${VAR:?message} here: a quote in its message hides the lines after it.)
modal_guard() {
  local p=${MODAL_PROFILE:-} ws denied
  if [ -z "$p" ]; then
    echo "modal: set MODAL_PROFILE to raea's profile (sdk/cloud/deploy/login.sh makes it)"
    return 2
  fi
  ws=$(modal profile list --json 2>/dev/null | python3 -c 'import json, sys; print(next((x["workspace"] for x in json.load(sys.stdin) if x["name"] == sys.argv[1]), ""))' "$p")
  if [ "$ws" != "$p" ]; then
    echo "modal: profile $p reaches workspace ${ws:-none}, not $p"
    return 2
  fi
  # a new workspace builds images with Modal's newest builder, which this machine's client (1.5.2) can't speak; 2025.06
  # is the newest it can, and what the farm was measured on
  export MODAL_IMAGE_BUILDER_VERSION=${MODAL_IMAGE_BUILDER_VERSION:-2025.06}
  denied=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel 2>/dev/null)/private/cloud/modal-denied.txt
  if [ -f "$denied" ] && grep -qxF "$ws" "$denied"; then
    echo "modal: $ws belongs to another business ($denied)"
    return 2
  fi
}
