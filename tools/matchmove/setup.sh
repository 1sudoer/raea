#!/bin/sh
# matchmove's setup on a Mac or Linux: setup.py beside this (what it installs, where, how big), run through uv as raea runs it
# on the model's first use (raea_io_assets::run_setup). On Windows:
#   uv run --no-project --python 3.11 python -I tools\matchmove\setup.py
# uv: $UV, the one on the PATH, raea's own (~/.cache/raea-tools/uv), else on an Apple-silicon Mac tools/ensure.sh
# fetches that one now.
#   tools/matchmove/setup.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
own=${RAEA_TOOLS_CACHE:-$HOME/.cache/raea-tools}/uv/uv
UV=${UV:-$(command -v uv || true)}
[ -n "$UV" ] || [ ! -x "$own" ] || UV=$own
[ -n "$UV" ] || [ "$(uname -sm)" != "Darwin arm64" ] || UV=$("$here/../ensure.sh" uv)
[ -n "$UV" ] || { echo "matchmove setup: needs uv: install it (https://docs.astral.sh/uv), or let raea fetch its own on the model's first use" >&2; exit 1; }
export UV
exec "$UV" run -q --no-project --python 3.11 python -I "$here/setup.py" "$@"
