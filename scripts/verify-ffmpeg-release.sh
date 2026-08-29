#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
runtime_dir="$repository_root/src-tauri/resources/audio-tools"

for binary in ffmpeg ffprobe; do
  path="$runtime_dir/bin/$binary"
  if [[ ! -x "$path" ]]; then
    echo "Missing executable $binary release runtime. Run: npm run ffmpeg:runtime" >&2
    exit 1
  fi
  if [[ "$(file -b "$path")" != *"Mach-O 64-bit executable arm64"* ]]; then
    echo "$binary is not an arm64 macOS executable." >&2
    exit 1
  fi
  minimum_version="$(otool -l "$path" | awk '/LC_BUILD_VERSION/{found=1; next} found && /minos/{print $2; exit}')"
  if [[ "$minimum_version" != "14.0" ]]; then
    echo "$binary targets macOS $minimum_version instead of macOS 14.0." >&2
    exit 1
  fi
done

test -s "$runtime_dir/manifest.json"
grep -q '"ffmpegVersion": "8.0.3"' "$runtime_dir/manifest.json"
grep -q '"lameVersion": "3.100"' "$runtime_dir/manifest.json"
grep -q '"minimumMacosVersion": "14.0"' "$runtime_dir/manifest.json"
test -s "$runtime_dir/licenses/FFmpeg-LGPL-2.1.txt"
test -s "$runtime_dir/licenses/LAME-LGPL.txt"
"$runtime_dir/bin/ffmpeg" -hide_banner -version >/dev/null
"$runtime_dir/bin/ffprobe" -hide_banner -version >/dev/null
"$runtime_dir/bin/ffmpeg" -hide_banner -L | grep -q "GNU Lesser General Public"
if otool -L "$runtime_dir/bin/ffmpeg" "$runtime_dir/bin/ffprobe" \
  | grep -E '/opt/homebrew|/usr/local'; then
  echo "The FFmpeg runtime depends on a package-manager path." >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/sonarcan-ffmpeg-verify.XXXXXX")"
trap 'rm -rf "$smoke_dir"' EXIT
"$runtime_dir/bin/ffmpeg" -hide_banner -loglevel error \
  -f lavfi -i "sine=frequency=440:duration=0.1" -c:a aac "$smoke_dir/input.m4a"
"$runtime_dir/bin/ffmpeg" -hide_banner -loglevel error \
  -i "$smoke_dir/input.m4a" -c:a libmp3lame -q:a 0 "$smoke_dir/output.mp3"
codec_name="$("$runtime_dir/bin/ffprobe" -v error -select_streams a:0 \
  -show_entries stream=codec_name -of default=nw=1:nk=1 "$smoke_dir/output.mp3")"
if [[ "$codec_name" != "mp3" ]]; then
  echo "The embedded FFmpeg runtime failed its AAC-to-MP3 conversion test." >&2
  exit 1
fi
echo "Bundled FFmpeg and FFprobe are ready."
