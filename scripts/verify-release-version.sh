#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
package_version="$(node -p "require('$repository_root/package.json').version")"
tauri_version="$(node -p "require('$repository_root/src-tauri/tauri.conf.json').version")"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repository_root/src-tauri/Cargo.toml" | head -1)"
release_tag="${GITHUB_REF_NAME:-v$package_version}"

if [[ "$package_version" != "$tauri_version" || "$package_version" != "$cargo_version" ]]; then
  echo "package.json, tauri.conf.json and Cargo.toml versions must match." >&2
  exit 1
fi
if [[ "$release_tag" != "v$package_version" ]]; then
  echo "Release tag $release_tag must equal v$package_version." >&2
  exit 1
fi
echo "Release version $package_version is consistent."
