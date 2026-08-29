#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
app_bundle="$repository_root/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/SonArcan.app"
launch_services="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
info_plist="$app_bundle/Contents/Info.plist"

if [[ ! -d "$app_bundle" ]]; then
  echo "Missing SonArcan.app. Run: npm run build:macos:app" >&2
  exit 1
fi
if [[ ! -x "$launch_services" ]]; then
  echo "macOS Launch Services registration tool is unavailable." >&2
  exit 1
fi
if [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleDocumentTypes:0:LSTypeIsPackage' "$info_plist")" != "true" ]]; then
  echo "SonArcan.app does not declare .sac as a macOS document package." >&2
  exit 1
fi
if [[ "$(/usr/libexec/PlistBuddy -c 'Print :UTExportedTypeDeclarations:0:UTTypeIdentifier' "$info_plist")" != "music.sonarcan.project" ]]; then
  echo "SonArcan.app does not export the expected .sac document type." >&2
  exit 1
fi

"$launch_services" -f "$app_bundle"
echo "Registered $app_bundle for .sac project packages."
