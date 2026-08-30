#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
runtime="$repository_root/src-tauri/resources/mlx-runtime/runtime/bin/python3.13"
model_dir="$repository_root/src-tauri/resources/models/demucs-mlx"

if [[ ! -x "$runtime" ]]; then
  echo "Missing executable release MLX runtime. Run: npm run mlx:runtime" >&2
  exit 1
fi
"$runtime" -m sonarcan_mlx_worker self-test --model-dir "$model_dir"
