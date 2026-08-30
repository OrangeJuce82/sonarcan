#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
runtime="$repository_root/src-tauri/resources/chord-runtime/runtime/bin/python3.12"

if [[ ! -x "$runtime" ]]; then
  echo "Missing executable LV-Chordia runtime. Run: npm run chords:runtime" >&2
  exit 1
fi
"$runtime" -m sonarcan_chord_worker.worker --self-test
