#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
model_dir="$repository_root/src-tauri/resources/models/beat-this"
model_path="$model_dir/final0.ckpt"
expected_sha256="8c328b45f59d8dd3dff219253ff6a8d6482be57d0133a29140e2febbf8eb8331"
model_url="https://cloud.cp.jku.at/public.php/dav/files/7ik4RrBKTS273gp/final0.ckpt"

mkdir -p "$model_dir"
if [[ ! -f "$model_path" ]]; then
  temporary="$(mktemp "$model_dir/.final0.XXXXXX")"
  trap 'rm -f "$temporary"' EXIT
  curl --fail --location --output "$temporary" "$model_url"
  actual_sha256="$(shasum -a 256 "$temporary" | cut -d ' ' -f 1)"
  if [[ "$actual_sha256" != "$expected_sha256" ]]; then
    echo "Beat This! final0 checkpoint failed SHA-256 verification." >&2
    exit 1
  fi
  mv "$temporary" "$model_path"
  trap - EXIT
fi

actual_sha256="$(shasum -a 256 "$model_path" | cut -d ' ' -f 1)"
if [[ "$actual_sha256" != "$expected_sha256" ]]; then
  echo "Beat This! final0 checkpoint failed SHA-256 verification." >&2
  exit 1
fi
echo "Pinned Beat This! final0 checkpoint is ready in $model_dir"
