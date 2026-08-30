#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
runtime="$repository_root/src-tauri/resources/chord-runtime/runtime/bin/python3.12"

if [[ ! -x "$runtime" ]]; then
  echo "Missing executable LV-Chordia runtime. Run: npm run chords:runtime" >&2
  exit 1
fi
self_test="$("$runtime" -m sonarcan_chord_worker.worker --self-test)"
echo "$self_test"
SONARCAN_CHORD_SELF_TEST="$self_test" node -e '
  const result = JSON.parse(process.env.SONARCAN_CHORD_SELF_TEST ?? "{}");
  const expected = ["complete", "essential", "standard"];
  if (!result.ok || JSON.stringify(result.modes) !== JSON.stringify(expected)) {
    console.error("The bundled LV-Chordia worker is stale or has an invalid three-mode contract. Run: npm run chords:runtime");
    process.exit(1);
  }
'
