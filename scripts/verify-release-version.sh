#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
package_version="$(node -p "require('$repository_root/package.json').version")"
tauri_version="$(node -p "require('$repository_root/src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repository_root/src-tauri/Cargo.toml" | head -1)"
release_tag="${GITHUB_REF_NAME:-v$package_version}"
release_workflow="$repository_root/.github/workflows/release-macos.yml"

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
configured_icons="$(node -p "require('$repository_root/src-tauri/tauri.conf.json').bundle.icon.join('\\n')")"
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
echo "Release version $package_version is consistent."
