#!/usr/bin/env bash
# FFmpeg for Windows x64 as raea ships it (beside raea.exe) and fetches it (io/assets/src/setup.rs): FFmpeg's own
# release, cross-built with mingw-w64 on Linux, static, with only FFmpeg's own code, zlib and libvpx's decoders, so it
# is LGPL 2.1 or later (no --enable-gpl: no x264, x265 or other GPL library) and its complete source is the tarballs and this
# script. raea uses it to read what its own decoders can't (H.264 with B-frames, HEVC, VP9 in a phone's or a camera's
# video) and to probe; it encodes with its own encoders, so no encoder library is needed.
#   tools/release/ffmpeg-win.sh OUT_DIR      (Ubuntu: apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64 libz-mingw-w64-dev nasm make pkg-config xz-utils zip)
# writes OUT_DIR/ffmpeg-$v-lgpl-win64.zip and its .sha256; .github/workflows/tools-ffmpeg.yml runs it and publishes the
# zip with the source tarball, its signature and this script.
set -euo pipefail
die() { echo "ffmpeg-win: $*" >&2; exit 1; }
out=$(mkdir -p "${1:?OUT_DIR}" && cd "$1" && pwd)
v=9.0.2
# FFmpeg's release tarball, signed by its release key (FCF986EA15E6E293A5644F10B4322F04D67658D8; checked 11-19)
sum=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
name=ffmpeg-$v-lgpl-win64
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
curl -fsSL --retry 5 --retry-all-errors -o "ffmpeg-$v.tar.xz" "https://ffmpeg.org/releases/ffmpeg-$v.tar.xz" || die "couldn't download FFmpeg $v"
echo "$sum  ffmpeg-$v.tar.xz" | sha256sum -c - >/dev/null || die "ffmpeg-$v.tar.xz isn't the tarball pinned here"
tar -xJf "ffmpeg-$v.tar.xz"
# libvpx's release (BSD 3-clause, with the WebM project's patent grant), decoders only: FFmpeg's own VP8 and VP9
# decoders drop the alpha a WebM keeps beside its picture, so raea decodes such a stream with libvpx (io/ffmpeg, #113)
vpx=1.17.0
vpx_sum=1020f184046187baa2985dbde38e0691f49c44088bca7a1842b0236c6081dc0a
curl -fsSL --retry 5 --retry-all-errors -o "libvpx-$vpx.tar.gz" "https://github.com/webmproject/libvpx/archive/refs/tags/v$vpx.tar.gz" || die "couldn't download libvpx $vpx"
echo "$vpx_sum  libvpx-$vpx.tar.gz" | sha256sum -c - >/dev/null || die "libvpx-$vpx.tar.gz isn't the tarball pinned here"
tar -xzf "libvpx-$vpx.tar.gz"
vconf=(--target=x86_64-win64-gcc --prefix="$work/vpx" --enable-static --disable-shared --disable-examples --disable-tools
  --disable-docs --disable-unit-tests --disable-vp8-encoder --disable-vp9-encoder --as=nasm)
(cd "libvpx-$vpx" && CROSS=x86_64-w64-mingw32- ./configure "${vconf[@]}" && make -j"$(nproc)" && make install) >"$work/vpx.log" 2>&1 || { tail -30 "$work/vpx.log"; die "libvpx failed"; }
export PKG_CONFIG_PATH="$work/vpx/lib/pkgconfig"
cd "ffmpeg-$v"
# --disable-autodetect keeps out every library the build machine happens to have; threads and zlib are asked for by
# name. No network (raea reads files), no capture devices (a webcam's vfwcap, a screen's gdigrab: raea reads files,
# and they pull in Windows' capture DLLs), no ffplay, no hardware decoders (raea decodes on the CPU here). Media
# Foundation for its H.264 encoder (h264_mf, Windows' own, which FFmpeg loads at run time): FFmpeg has no H.264
# encoder of its own and libx264 is GPL, and raea cuts a clip a person plays to H.264 (io/ffmpeg's h264()). Its
# encoder is written against D3D11's types (libavcodec/mfenc.c), so D3D11VA comes with it; raea asks it for no
# hardware decoding.
conf=(--target-os=mingw32 --arch=x86_64 --cross-prefix=x86_64-w64-mingw32- --enable-cross-compile
  --pkg-config=pkg-config --pkg-config-flags=--static --disable-shared --enable-static
  --disable-debug --disable-doc --disable-ffplay --disable-network --disable-devices --disable-autodetect
  --enable-w32threads --enable-zlib --enable-libvpx --disable-encoder=libvpx_vp8,libvpx_vp9 --enable-mediafoundation --enable-d3d11va
  --extra-cflags="-I$work/vpx/include" --extra-ldflags="-static -L$work/vpx/lib")
./configure "${conf[@]}" >"$work/configure.log" 2>&1 || { tail -30 "$work/configure.log"; die "configure failed"; }
grep -q '#define FFMPEG_LICENSE "LGPL version 2.1 or later"' config.h || die "the build isn't LGPL 2.1 or later: $(grep FFMPEG_LICENSE config.h)"
for c in LIBVPX_VP8_DECODER LIBVPX_VP9_DECODER H264_MF_ENCODER; do
  grep -q "#define CONFIG_$c 1" config_components.h || die "the build has no $c (configure.log: $(grep -iE 'vpx|mediafoundation' "$work/configure.log" | tail -3))"
done
make -j"$(nproc)" >"$work/make.log" 2>&1 || { tail -30 "$work/make.log"; die "make failed"; }
x86_64-w64-mingw32-strip ffmpeg.exe ffprobe.exe
# only Windows' own DLLs: nothing of mingw's (libwinpthread, libgcc) or the build machine's
for f in ffmpeg.exe ffprobe.exe; do
  bad=$(x86_64-w64-mingw32-objdump -p "$f" | awk '/DLL Name:/ {print tolower($3)}' | grep -vE '^(kernel32|user32|advapi32|bcrypt|ole32|oleaut32|shell32|shlwapi|psapi|msvcrt|ws2_32|secur32|gdi32|vfw32|api-ms-win-.*)\.dll$' || true)
  [ -z "$bad" ] || die "$f imports $bad"
done
mkdir -p "$work/$name/bin"
cp ffmpeg.exe ffprobe.exe "$work/$name/bin/"
cp COPYING.LGPLv2.1 "$work/$name/LICENSE"
cp "../libvpx-$vpx/LICENSE" "$work/$name/libvpx-LICENSE"
cp "../libvpx-$vpx/PATENTS" "$work/$name/libvpx-PATENTS"
{
  echo "FFmpeg $v for Windows x64, as raea ships and fetches it."
  echo
  echo "Built from FFmpeg's own release, ffmpeg-$v.tar.xz (sha256 $sum, signed by FFmpeg's release key), with mingw-w64,"
  echo "with FFmpeg's own code, zlib and libvpx $vpx's decoders (libvpx-$vpx.tar.gz, sha256 $vpx_sum). FFmpeg is licensed"
  echo "under the GNU Lesser General Public License version 2.1 or later (LICENSE here); zlib under the zlib licence;"
  echo "libvpx under the BSD 3-clause licence with the WebM project's patent grant (libvpx-LICENSE, libvpx-PATENTS)."
  echo "This build has no GPL part. Its H.264 encoder is Windows' own, through Media Foundation."
  echo
  echo "Its complete source is those tarballs and the script that built it (tools/release/ffmpeg-win.sh in"
  echo "https://github.com/shinyobjectz/raea), all published beside this build:"
  echo "  https://github.com/shinyobjectz/raea/releases/tag/tools-ffmpeg-$v-lgpl"
  echo
  echo "libvpx configure ${vconf[*]}"
  echo "configure ${conf[*]}"
} > "$work/$name/README.txt"
(cd "$work" && rm -f "$out/$name.zip" && zip -qr -X "$out/$name.zip" "$name")
(cd "$out" && sha256sum "$name.zip" > "$name.zip.sha256")
cp "$work/ffmpeg-$v.tar.xz" "$work/libvpx-$vpx.tar.gz" "$out/"
echo "ffmpeg-win: $(cat "$out/$name.zip.sha256"), $(wc -c < "$out/$name.zip" | tr -d ' ') bytes; ffmpeg.exe $(wc -c < "$work/$name/bin/ffmpeg.exe" | tr -d ' ') bytes"
