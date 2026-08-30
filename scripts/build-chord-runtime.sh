#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
worker_root="$repository_root/tools/sonarcan-chord-worker"
runtime_dir="$repository_root/src-tauri/resources/chord-runtime/runtime"
downbeat_model="$repository_root/src-tauri/resources/models/beat-this/final0.ckpt"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "The LV-Chordia release runtime must be assembled on an Apple-silicon Mac." >&2
  exit 1
fi
mkdir -p "$(dirname "$runtime_dir")"
if [[ -x "$runtime_dir/bin/python3.12" ]]; then
  uv export --quiet --project "$worker_root" --locked --no-dev --no-editable --output-file "$runtime_dir/requirements.lock.txt"
  (
    cd "$worker_root"
    uv pip sync --system --break-system-packages --python "$runtime_dir/bin/python3.12" --reinstall-package sonarcan-lv-chordia-worker "$runtime_dir/requirements.lock.txt"
  )
  "$runtime_dir/bin/python3.12" -m sonarcan_chord_worker.worker --self-test --downbeat-model "$downbeat_model"
  echo "Pinned LV-Chordia runtime refreshed in $runtime_dir"
  exit 0
fi
if [[ -e "$runtime_dir" ]]; then
  echo "The existing LV-Chordia runtime is incomplete. Run npm run clean before rebuilding it." >&2
  exit 1
fi

staging_root="$(mktemp -d "$(dirname "$runtime_dir")/.runtime-build.XXXXXX")"
staging_runtime="$staging_root/runtime"
cleanup() { rm -rf "$staging_root"; }
trap cleanup EXIT
managed_python="$(uv python find --managed-python 3.12.12)"
managed_root="$(cd "$(dirname "$managed_python")/.." && pwd)"
cp -R "$managed_root" "$staging_runtime"
find "$staging_runtime" -name .DS_Store -type f -delete
uv export --quiet --project "$worker_root" --locked --no-dev --no-editable --output-file "$staging_runtime/requirements.lock.txt"
(
  cd "$worker_root"
  uv pip sync --system --break-system-packages --python "$staging_runtime/bin/python3.12" --reinstall-package sonarcan-lv-chordia-worker "$staging_runtime/requirements.lock.txt"
)
"$staging_runtime/bin/python3.12" -m sonarcan_chord_worker.worker --self-test --downbeat-model "$downbeat_model"
mv "$staging_runtime" "$runtime_dir"
trap - EXIT
rmdir "$staging_root"
echo "Pinned LV-Chordia runtime assembled in $runtime_dir"
