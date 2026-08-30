#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
worker_root="$repository_root/tools/sonarcan-mlx-worker"
model_dir="$repository_root/src-tauri/resources/models/demucs-mlx"

mkdir -p "$model_dir"
uv run --project "$worker_root" --locked --extra model-build \
  python -m sonarcan_mlx_worker.build_model --output-dir "$model_dir"
uv run --project "$worker_root" --locked \
  python -m sonarcan_mlx_worker self-test --model-dir "$model_dir"
