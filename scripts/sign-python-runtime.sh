#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
python_runtime_dir="${SONARCAN_PYTHON_RUNTIME_DIR:-$repository_root/src-tauri/resources/python-runtime/runtime}"
audio_tools_dir="$repository_root/src-tauri/resources/audio-tools/bin"
identity="${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY is required}"
signed_count=0
resource_roots=("$python_runtime_dir" "$audio_tools_dir")

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
if [[ -d "$python_runtime_dir/lib/python3.13/site-packages/sonarcan_mlx_worker" ]]; then
  refresher=("$python_runtime_dir/bin/python3.13" -m sonarcan_mlx_worker.refresh_records)
elif [[ -d "$python_runtime_dir/lib/python3.13/site-packages/sonarcan_torch_worker" ]]; then
  refresher=("$python_runtime_dir/bin/python3.13" -m sonarcan_torch_worker.refresh_records)
else
  refresher=()
fi
if [[ "${#refresher[@]}" -gt 0 ]]; then
  "${refresher[@]}" "$python_runtime_dir/lib/python3.13/site-packages"
fi
echo "Signed and verified $signed_count release resource binaries."
