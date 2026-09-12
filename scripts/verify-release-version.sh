#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repository_root"
package_version="$(node -p "require('./package.json').version")"
tauri_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repository_root/src-tauri/Cargo.toml" | head -1)"
release_tag="${GITHUB_REF_NAME:-v$package_version}"
release_workflow="$repository_root/.github/workflows/release-desktop.yml"

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
if ! grep -Fq 'cancel-in-progress: true' "$release_workflow" \
  || ! grep -Fq 'draft_ids="$(' "$release_workflow" \
  || ! grep -Fq 'select(.draft and .tag_name' "$release_workflow" \
  || ! grep -Fq 'gh api --method DELETE "repos/$GITHUB_REPOSITORY/releases/$draft_id"' "$release_workflow" \
  || grep -Fq -- '--slurp' "$release_workflow"; then
  echo "Release retries must cancel stale runs and recreate only the draft for their tag." >&2
  exit 1
fi
if [[ "$(grep -Ec '^  release-' "$release_workflow")" -ne 2 ]]; then
  echo "The release workflow must contain exactly the macOS and Linux jobs." >&2
  exit 1
fi
if ! grep -Fq 'npm run verify:audio-tools-source' "$release_workflow" \
  || [[ "$(grep -Fc 'npm run verify:native-release' "$release_workflow")" -lt 2 ]]; then
  echo "Every release must verify its native runtime sources." >&2
  exit 1
fi
if grep -Fq 'npm run python:runtime' "$release_workflow" \
  || grep -Fq 'src-tauri/resources/python-runtime' "$release_workflow"; then
  echo "Release packaging must never assemble or inspect a Python/PyTorch runtime." >&2
  exit 1
fi
if ! grep -Fq 'release-linux:' "$release_workflow" \
  || ! grep -Fq 'x86_64-unknown-linux-gnu' "$release_workflow" \
  || ! grep -Fq 'aarch64-unknown-linux-gnu' "$release_workflow"; then
  echo "The release workflow must publish Linux x86_64 and arm64 editions." >&2
  exit 1
fi
if grep -Eiq '(^|[^[:alnum:]_])rpm([^[:alnum:]_]|$)' "$release_workflow"; then
  echo "The release workflow must not build or verify RPM packages." >&2
  exit 1
fi
if ! grep -Fq -- '--notes-file RELEASE_NOTES.md' "$release_workflow"; then
  echo "The release workflow must publish the curated edition notes." >&2
  exit 1
fi
if [[ "$(grep -Fc 'npm run verify:lightweight-bundle' "$release_workflow")" -ne 2 ]]; then
  echo "Every release package must pass the Python-free 512 MiB bundle gate." >&2
  exit 1
fi
if grep -Fq 'split -b' "$release_workflow"; then
  echo "Native Linux packages must remain below the bundle limit and must not be split." >&2
  exit 1
fi
echo "Release version $package_version is consistent."
