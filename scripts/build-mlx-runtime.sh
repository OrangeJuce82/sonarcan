#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
worker_root="$repository_root/tools/sonarcan-mlx-worker"
runtime_dir="$repository_root/src-tauri/resources/mlx-runtime/runtime"
model_dir="$repository_root/src-tauri/resources/models/demucs-mlx"
model_source="${SONARCAN_MLX_MODEL_DIR:-$model_dir}"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "The MLX release runtime must be assembled on an Apple-silicon Mac." >&2
  exit 1
fi
if [[ ! -f "$model_source/htdemucs_6s.safetensors" || ! -f "$model_source/htdemucs_6s_config.json" ]]; then
  echo "Missing pinned htdemucs_6s model in $model_source" >&2
  exit 1
fi

rm -rf "$runtime_dir"
mkdir -p "$(dirname "$runtime_dir")" "$model_dir"
managed_python="$(uv python find --managed-python 3.13.5)"
managed_root="$(cd "$(dirname "$managed_python")/.." && pwd)"
cp -R "$managed_root" "$runtime_dir"
find "$runtime_dir" -name .DS_Store -type f -delete
uv export --quiet --project "$worker_root" --locked --no-dev --no-editable \
  --output-file "$runtime_dir/requirements.lock.txt"
(
  cd "$worker_root"
  uv pip sync --system --break-system-packages --refresh-package sonarcan-mlx-worker \
    --python "$runtime_dir/bin/python3.13" \
    "$runtime_dir/requirements.lock.txt"
)
if [[ "$model_source" != "$model_dir" ]]; then
  cp "$model_source/htdemucs_6s.safetensors" "$model_dir/htdemucs_6s.safetensors"
  cp "$model_source/htdemucs_6s_config.json" "$model_dir/htdemucs_6s_config.json"
fi
"$runtime_dir/bin/python3.13" -m sonarcan_mlx_worker self-test --model-dir "$model_dir"

mkdir -p "$repository_root/release-artifacts"
tar -C "$repository_root/src-tauri/resources" -czf \
  "$repository_root/release-artifacts/sonarcan-mlx-runtime-macos-arm64-python-3.13.5.tar.gz" \
  mlx-runtime models/demucs-mlx
echo "Pinned MLX runtime assembled in $runtime_dir"
