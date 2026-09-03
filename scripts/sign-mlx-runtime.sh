#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
mlx_runtime_dir="$repository_root/src-tauri/resources/mlx-runtime/runtime"
stem_runtime_dir="$repository_root/src-tauri/resources/stem-runtime/runtime"
chord_runtime_dir="$repository_root/src-tauri/resources/chord-runtime/runtime"
audio_tools_dir="$repository_root/src-tauri/resources/audio-tools/bin"
identity="${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY is required}"
signed_count=0
resource_roots=("$chord_runtime_dir" "$audio_tools_dir")
if [[ -d "$mlx_runtime_dir" ]]; then resource_roots+=("$mlx_runtime_dir"); fi
if [[ -d "$stem_runtime_dir" ]]; then resource_roots+=("$stem_runtime_dir"); fi

while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q "Mach-O"; then
    if [[ "$identity" == "-" ]]; then
      # Ad-hoc signatures have no Team ID, so hardened runtime would reject
      # libpython as a separately mapped identity. Developer ID release
      # signatures below share one Team ID and do use hardened runtime.
      codesign --force --sign - "$candidate"
    else
      codesign --force --sign "$identity" --options runtime --timestamp "$candidate"
    fi
    codesign --verify --strict "$candidate"
    signed_count=$((signed_count + 1))
  fi
done < <(find "${resource_roots[@]}" -type f -print0)

if [[ "$signed_count" -eq 0 ]]; then
  echo "No Mach-O file was found in the release resources." >&2
  exit 1
fi
if [[ -x "$mlx_runtime_dir/bin/python3.13" ]]; then
  refresher=("$mlx_runtime_dir/bin/python3.13" -m sonarcan_mlx_worker.refresh_records)
  "${refresher[@]}" "$mlx_runtime_dir/lib/python3.13/site-packages"
else
  refresher=("$stem_runtime_dir/bin/python3.12" -m sonarcan_torch_worker.refresh_records)
  "${refresher[@]}" "$stem_runtime_dir/lib/python3.12/site-packages"
fi
"${refresher[@]}" "$chord_runtime_dir/lib/python3.12/site-packages"
echo "Signed and verified $signed_count release resource binaries."
