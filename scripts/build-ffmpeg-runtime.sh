#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
output_dir="$repository_root/src-tauri/resources/audio-tools"
ffmpeg_version="8.0.3"
ffmpeg_sha256="6136812ea6d4e68bdba27e33c2a94382711cdf4f8602ffef056ff792bd6f9818"
lame_version="3.100"
lame_sha256="ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e"
minimum_macos_version="14.0"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "The bundled FFmpeg runtime must be assembled on an Apple-silicon Mac." >&2
  exit 1
fi

build_root="$(mktemp -d "${TMPDIR:-/tmp}/sonarcan-ffmpeg.XXXXXX")"
trap 'rm -rf "$build_root"' EXIT
prefix="$build_root/prefix"
mkdir -p "$prefix" "$output_dir/bin" "$output_dir/licenses"
export MACOSX_DEPLOYMENT_TARGET="$minimum_macos_version"

download_and_verify() {
  local url="$1"
  local destination="$2"
  local expected_sha256="$3"
  curl --fail --location --silent --show-error --proto '=https' --tlsv1.2 \
    "$url" --output "$destination"
  local actual_sha256
  actual_sha256="$(shasum -a 256 "$destination" | awk '{print $1}')"
  if [[ "$actual_sha256" != "$expected_sha256" ]]; then
    echo "Checksum mismatch for $destination" >&2
    exit 1
  fi
}

lame_archive="$build_root/lame-$lame_version.tar.gz"
download_and_verify \
  "https://downloads.sourceforge.net/project/lame/lame/$lame_version/lame-$lame_version.tar.gz" \
  "$lame_archive" "$lame_sha256"
tar -xzf "$lame_archive" -C "$build_root"
(
  cd "$build_root/lame-$lame_version"
  ./configure \
    --prefix="$prefix" \
    --disable-shared \
    --enable-static \
    --disable-frontend
  make -j "$(sysctl -n hw.logicalcpu)"
  make install
)

ffmpeg_archive="$build_root/ffmpeg-$ffmpeg_version.tar.xz"
download_and_verify \
  "https://ffmpeg.org/releases/ffmpeg-$ffmpeg_version.tar.xz" \
  "$ffmpeg_archive" "$ffmpeg_sha256"
tar -xJf "$ffmpeg_archive" -C "$build_root"
(
  cd "$build_root/ffmpeg-$ffmpeg_version"
  PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure \
    --prefix="$prefix" \
    --arch=arm64 \
    --target-os=darwin \
    --cc=clang \
    --disable-shared \
    --enable-static \
    --disable-autodetect \
    --disable-debug \
    --disable-doc \
    --disable-ffplay \
    --disable-network \
    --enable-libmp3lame \
    --extra-cflags="-I$prefix/include" \
    --extra-ldflags="-L$prefix/lib"
  make -j "$(sysctl -n hw.logicalcpu)" ffmpeg ffprobe
)

cp "$build_root/ffmpeg-$ffmpeg_version/ffmpeg" "$output_dir/bin/ffmpeg"
cp "$build_root/ffmpeg-$ffmpeg_version/ffprobe" "$output_dir/bin/ffprobe"
strip -x "$output_dir/bin/ffmpeg" "$output_dir/bin/ffprobe"
chmod 755 "$output_dir/bin/ffmpeg" "$output_dir/bin/ffprobe"
cp "$build_root/ffmpeg-$ffmpeg_version/COPYING.LGPLv2.1" \
  "$output_dir/licenses/FFmpeg-LGPL-2.1.txt"
cp "$build_root/lame-$lame_version/COPYING" "$output_dir/licenses/LAME-LGPL.txt"

cat > "$output_dir/manifest.json" <<EOF
{
  "architecture": "arm64",
  "ffmpegVersion": "$ffmpeg_version",
  "ffmpegSourceSha256": "$ffmpeg_sha256",
  "lameVersion": "$lame_version",
  "lameSourceSha256": "$lame_sha256",
  "minimumMacosVersion": "$minimum_macos_version"
}
EOF

"$output_dir/bin/ffmpeg" -hide_banner -version | head -n 1
"$output_dir/bin/ffprobe" -hide_banner -version | head -n 1
file "$output_dir/bin/ffmpeg" "$output_dir/bin/ffprobe"
if otool -L "$output_dir/bin/ffmpeg" "$output_dir/bin/ffprobe" | grep -E "$build_root|/opt/homebrew|/usr/local"; then
  echo "The FFmpeg runtime contains a non-relocatable dependency." >&2
  exit 1
fi
echo "Pinned FFmpeg runtime assembled in $output_dir"
