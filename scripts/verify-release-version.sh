#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repository_root"
package_version="$(node -p "require('./package.json').version")"
tauri_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repository_root/src-tauri/Cargo.toml" | head -1)"
release_tag="${GITHUB_REF_NAME:-v$package_version}"
release_workflow="$repository_root/.github/workflows/release-macos.yml"
portable_worker="$repository_root/tools/sonarcan-torch-worker/pyproject.toml"
shared_runtime="$repository_root/tools/sonarcan-python-runtime/uv.lock"

if [[ "$package_version" != "$tauri_version" || "$package_version" != "$cargo_version" ]]; then
  echo "package.json, tauri.conf.json and Cargo.toml versions must match." >&2
  exit 1
fi
if [[ "$release_tag" != "v$package_version" ]]; then
  echo "Release tag $release_tag must equal v$package_version." >&2
  exit 1
fi

required_icons=(
  "icons/32x32.png"
  "icons/128x128.png"
  "icons/128x128@2x.png"
  "icons/icon.icns"
  "icons/icon.ico"
)
configured_icons="$(node -p "require('./src-tauri/tauri.conf.json').bundle.icon.join('\\n')")"
for icon in "${required_icons[@]}"; do
  if ! grep -Fxq "$icon" <<< "$configured_icons"; then
    echo "The release bundle is missing the configured app icon $icon." >&2
    exit 1
  fi
  if [[ ! -s "$repository_root/src-tauri/$icon" ]]; then
    echo "The release app icon $icon is missing or empty." >&2
    exit 1
  fi
done
if ! grep -Fq 'APPLE_SIGNING_IDENTITY: "-"' "$release_workflow"; then
  echo "The macOS release workflow must explicitly use the ad-hoc signing identity." >&2
  exit 1
fi
if grep -Eq 'secrets\.APPLE_(CERTIFICATE|CERTIFICATE_PASSWORD|ID|PASSWORD|TEAM_ID)' "$release_workflow"; then
  echo "The ad-hoc macOS release workflow must not require paid Apple credentials." >&2
  exit 1
fi
if ! grep -Fq 'SHA256SUMS.txt' "$release_workflow"; then
  echo "The macOS release workflow must publish a DMG checksum." >&2
  exit 1
fi
if ! grep -Fq '3.13.5' "$release_workflow"; then
  echo "The release workflow must install the shared Python runtime version." >&2
  exit 1
fi
if grep -Fq -- '--bundles deb,appimage' "$release_workflow"; then
  echo "The Linux release must not let the unreliable AppImage bundler block the verified DEB." >&2
  exit 1
fi
if ! grep -Fq 'npm run chords:downbeat-model' "$release_workflow"; then
  echo "The macOS release workflow must download and verify the pinned Beat This! model." >&2
  exit 1
fi
if ! grep -Fq -- '--self-test --downbeat-model "$app_bundle/Contents/Resources/models/beat-this/final0.ckpt"' "$release_workflow"; then
  echo "The bundled chord/downbeat self-test must receive the bundled Beat This! model." >&2
  exit 1
fi
if ! grep -Fq -- '--accelerator-self-test' "$repository_root/scripts/verify-bundled-release.mjs" \
  || ! grep -Fq -- '"accelerator-self-test"' "$repository_root/scripts/verify-bundled-release.mjs"; then
  echo "The bundled Apple Silicon release must qualify both MPS and MLX accelerators." >&2
  exit 1
fi
if ! grep -Fq 'PYTHONDONTWRITEBYTECODE: "1"' "$release_workflow"; then
  echo "Bundled runtime verification must not write bytecode after signing." >&2
  exit 1
fi
if ! grep -Fq 'x86_64-apple-darwin' "$release_workflow" \
  || ! grep -Fq 'macos-15-intel' "$release_workflow" \
  || ! grep -Fq 'tauri.macos-intel-light.conf.json' "$release_workflow"; then
  echo "The release workflow must publish the supported Intel macOS Light bundle on an Intel runner." >&2
  exit 1
fi
if ! grep -Fq 'SONARCAN_EDITION: light' "$release_workflow" \
  || ! grep -Fq 'npm run python:light-runtime' "$release_workflow"; then
  echo "Portable release jobs must build the Light edition and its minimal runtime." >&2
  exit 1
fi
if ! grep -Fq 'torch==2.13.0+cpu; sys_platform == '\''linux'\'' or sys_platform == '\''win32'\''' "$portable_worker"; then
  echo "Linux and Windows releases must pin the exact portable CPU Torch build." >&2
  exit 1
fi
if ! grep -Fq 'version = "2.13.0+cpu"' "$shared_runtime" \
  || ! grep -Fq 'version = "2.11.0+cpu"' "$shared_runtime" \
  || ! grep -Fq 'source = { registry = "https://download.pytorch.org/whl/cpu" }' "$shared_runtime" \
  || grep -Fq 'name = "nvidia-' "$shared_runtime"; then
  echo "The shared runtime must resolve CPU-only Torch audio packages without NVIDIA packages on Linux and Windows." >&2
  exit 1
fi
echo "Release version $package_version is consistent."
