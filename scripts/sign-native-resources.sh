#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
audio_tools_dir="$repository_root/src-tauri/resources/audio-tools/bin"
executorch_runtime_dir="$repository_root/src-tauri/resources/executorch-runtime"
identity="${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY is required}"
signed_count=0
resource_roots=("$audio_tools_dir" "$executorch_runtime_dir")

while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q "Mach-O"; then
    if [[ "$identity" == "-" ]]; then
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
echo "Signed and verified $signed_count release resource binaries."
