#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repository_root"

package_version="$(node -p "require('./package.json').version")"
tauri_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' src-tauri/Cargo.toml | head -1)"
release_tag="${GITHUB_REF_NAME:-v$package_version}"
release_workflow="$repository_root/.github/workflows/release-desktop.yml"
ci_workflow="$repository_root/.github/workflows/ci.yml"

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
)
configured_icons="$(node -p "require('./src-tauri/tauri.conf.json').bundle.icon.join('\n')")"
for icon in "${required_icons[@]}"; do
  if ! grep -Fxq "$icon" <<< "$configured_icons" || [[ ! -s "src-tauri/$icon" ]]; then
    echo "The release app icon $icon is missing or not configured." >&2
    exit 1
  fi
done

if [[ "$(grep -Ec '^  release-' "$release_workflow")" -ne 1 ]]; then
  echo "The release workflow must contain exactly one macOS release job." >&2
  exit 1
fi
if ! grep -Fq 'runs-on: macos-15' "$release_workflow" \
  || ! grep -Fq -- '--target aarch64-apple-darwin' "$release_workflow" \
  || grep -Eiq '(windows-|ubuntu-|unknown-linux|pc-windows|SONARCAN_GPU_BACKEND|CUDA|ROCm)' "$release_workflow"; then
  echo "The release workflow must target only macOS Apple Silicon." >&2
  exit 1
fi
if grep -Eq 'runs-on: (ubuntu|windows|macos-[^ ]*-intel)' "$ci_workflow"; then
  echo "Every CI job must run on macOS Apple Silicon." >&2
  exit 1
fi
if ! grep -Fq 'APPLE_SIGNING_IDENTITY: "-"' "$release_workflow" \
  || grep -Eq 'secrets\.APPLE_(CERTIFICATE|CERTIFICATE_PASSWORD|ID|PASSWORD|TEAM_ID)' "$release_workflow"; then
  echo "The macOS release must use ad-hoc signing without paid Apple credentials." >&2
  exit 1
fi
if ! grep -Fq 'SHA256SUMS.txt' "$release_workflow" \
  || ! grep -Fq -- '--notes-file RELEASE_NOTES.md' "$release_workflow"; then
  echo "The release must publish curated notes and a DMG checksum." >&2
  exit 1
fi
if ! grep -Fq 'cancel-in-progress: true' "$release_workflow" \
  || ! grep -Fq 'select(.draft and .tag_name' "$release_workflow" \
  || ! grep -Fq 'gh api --method DELETE "repos/$GITHUB_REPOSITORY/releases/$draft_id"' "$release_workflow"; then
  echo "Release retries must recreate only the draft for their tag." >&2
  exit 1
fi
if ! grep -Fq 'npm run python:runtime' "$release_workflow" \
  || ! grep -Fq 'npm run chords:downbeat-model' "$release_workflow" \
  || ! grep -Fq 'npm run verify:bundled-release' "$release_workflow"; then
  echo "The macOS release must assemble and verify its pinned MLX/MPS runtime." >&2
  exit 1
fi
if ! grep -Fq -- 'load_ensemble(False,device=device)' scripts/verify-bundled-release.mjs \
  || ! grep -Fq -- '"accelerator-self-test"' scripts/verify-bundled-release.mjs; then
  echo "The bundle verifier must exercise MPS and MLX without CPU fallback." >&2
  exit 1
fi
if [[ -e src-tauri/tauri.nvidia-gpu.conf.json || -e src-tauri/tauri.amd-gpu.conf.json \
  || -d tools/sonarcan-python-runtime-cuda || -d tools/sonarcan-python-runtime-rocm \
  || -d tools/sonarcan-torch-worker ]]; then
  echo "Unsupported Windows/Linux build profiles must not exist." >&2
  exit 1
fi

echo "Release version $package_version is consistent and Apple Silicon-only."
