#!/usr/bin/env bash
set -euo pipefail

if xcrun --find metal >/dev/null 2>&1 && xcrun --find metallib >/dev/null 2>&1; then
  echo "Using the Metal compiler bundled with the selected Xcode"
  exit 0
fi

if xcodebuild -help 2>&1 | grep -q -- "-downloadComponent"; then
  xcodebuild -downloadComponent MetalToolchain
fi

xcrun --find metal
xcrun --find metallib
