#!/usr/bin/env bash
# The complete corresponding source of the Git for Windows the Windows app ships (dugite-native's build, pinned in the
# feed's host/tools.ts WIN_GIT and tools/raea-app/win/build.sh), as GPLv2 section 3 asks of whoever distributes it:
# each package its etc/package-versions.txt names, as the source package it was built from. Git for Windows' own
# builds (Git, its MSYS2 runtime, bash, curl ...) from its pacman repository's releases, the rest from MSYS2's source
# mirror. tools-git-sources.yml runs it and publishes the archive as the prerelease tools-git-<tag>-sources.
#   tools/release/git-sources.sh --list           each package and the source it resolves to (no download)
#   tools/release/git-sources.sh OUT_DIR          OUT_DIR/git-for-windows-<tag>-sources.tar and its .sha256
set -euo pipefail
die() { echo "git-sources: $*" >&2; exit 1; }
tag=v2.56.0-2
url=https://github.com/desktop/dugite-native/releases/download/$tag/dugite-native-v2.56.0-a44aeca-windows-x64.tar.gz
sum=f1e17876337d7be143e5d51976da36643d65528125cfbc14d57f171de0fd5f14
list=0; [ "${1:-}" = --list ] && list=1
out=${1:?--list or OUT_DIR}
[ "$list" = 1 ] || out=$(mkdir -p "$out" && cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL --retry 5 -o "$work/git.tgz" "$url" || die "couldn't download $url"
[ "$( (sha256sum "$work/git.tgz" 2>/dev/null || shasum -a 256 "$work/git.tgz") | cut -d' ' -f1)" = "$sum" ] || die "$url isn't the build pinned"
tar -xzf "$work/git.tgz" -C "$work" ./etc/package-versions.txt ./LICENSE.txt

# the source package (pacman's pkgbase) a split package comes from
base() {
  case $1 in
    gcc-libs|cc-libs|libatomic|libgcc|libgomp|libquadmath|libstdc++) echo gcc ;;
    heimdal-libs) echo heimdal ;;
    libintl|gettext-runtime) echo gettext ;;
    libopenssl) echo openssl ;;
    libpcre) echo pcre ;;
    libreadline) echo readline ;;
    libsqlite) echo sqlite ;;
    curl-winssl|curl-openssl-alternate) echo curl ;;
    git-credential-wincred|git-for-windows-addons|git-gui|git-perl|git-send-email|git-subtree|gitk) echo git ;;
    libssh2-wincng) echo libssh2 ;;
    libwinpthread) echo winpthreads ;;
    *) echo "$1" ;;
  esac
}

# Git for Windows' source packages: every .src.tar.gz its pacman repository has released, by name
gh api --paginate "repos/git-for-windows/pacman-repo/releases?per_page=100" \
  -q '.[].assets[] | select(.name | endswith(".src.tar.gz")) | [.name, .browser_download_url] | @tsv' > "$work/gfw.tsv" ||
  die "couldn't list git-for-windows/pacman-repo's releases (gh api)"
exists() { [ "$(curl -s -o /dev/null -w '%{http_code}' -I -L "$1")" = 200 ]; }

: > "$work/sources.tsv"; missing=""
while read -r pkg ver; do
  [ -n "$pkg" ] || continue
  if [[ $pkg == mingw-w64-ucrt-x86_64-* ]]; then
    b=mingw-w64-$(base "${pkg#mingw-w64-ucrt-x86_64-}"); mirror=https://repo.msys2.org/mingw/sources
  else
    b=$(base "$pkg"); mirror=https://repo.msys2.org/msys/sources
  fi
  src=$(awk -F'\t' -v n="$b-$ver.src.tar.gz" '$1 == n {print $2; exit}' "$work/gfw.tsv")
  [ -n "$src" ] || { exists "$mirror/$b-$ver.src.tar.zst" && src=$mirror/$b-$ver.src.tar.zst; } || true
  if [ -n "$src" ]; then
    grep -qF "$src" "$work/sources.tsv" || printf '%s\t%s\t%s\n' "$b-$ver" "$src" "$pkg" >> "$work/sources.tsv"
  else
    missing="$missing $pkg-$ver($b)"
  fi
done < <(tr -d '\r' < "$work/etc/package-versions.txt" | sort -u)

if [ "$list" = 1 ]; then
  column -t -s$'\t' "$work/sources.tsv"
  [ -z "$missing" ] || { echo "unresolved:$missing"; exit 1; }
  exit 0
fi
[ -z "$missing" ] || die "no source found for:$missing"
mkdir -p "$work/src"
while IFS=$'\t' read -r name src _; do
  curl -fsSL --retry 5 --retry-all-errors -o "$work/src/$(basename "$src")" "$src" || die "couldn't download $src"
done < "$work/sources.tsv"
cp "$work/etc/package-versions.txt" "$work/src/package-versions.txt"
cp "$work/LICENSE.txt" "$work/src/LICENSE.txt"
cut -f1,2 "$work/sources.tsv" > "$work/src/SOURCES.tsv"
{
  echo "The complete corresponding source of Git for Windows with Git LFS as raea's Windows app ships it:"
  echo "dugite-native $tag ($url, sha256 $sum), built by Git for Windows and GitHub from the packages its"
  echo "package-versions.txt names. Each package's source package is here, from Git for Windows' pacman repository"
  echo "or MSYS2's source mirror (SOURCES.tsv says which). Collected by tools/release/git-sources.sh in"
  echo "https://github.com/shinyobjectz/raea."
} > "$work/src/README.txt"
name=git-for-windows-$tag-sources
(cd "$work" && mv src "$name" && tar -cf "$out/$name.tar" "$name")
(cd "$out" && (sha256sum "$name.tar" 2>/dev/null || shasum -a 256 "$name.tar") > "$name.tar.sha256")
echo "git-sources: $out/$name.tar ($(wc -l < "$work/sources.tsv" | tr -d ' ') source packages, $(du -h "$out/$name.tar" | cut -f1))"
