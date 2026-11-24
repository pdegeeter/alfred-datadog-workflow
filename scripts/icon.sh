#!/usr/bin/env bash
#
# Regenerate the Alfred workflow icon (workflow/icon.png) from a macOS system
# icon. The workflow ships no artwork of its own (no Datadog logo): item icons
# also point at macOS system icons at runtime.
#
# Requires: sips (ships with macOS).
#
# Usage: scripts/icon.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

ICNS="/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericNetworkIcon.icns"

if [[ ! -f "$ICNS" ]]; then
  echo "error: $ICNS not found." >&2
  exit 1
fi

sips -s format png -Z 512 "$ICNS" --out workflow/icon.png >/dev/null

echo "==> Wrote workflow/icon.png ($(sips -g pixelWidth -g pixelHeight workflow/icon.png | awk '/pixel/{print $2}' | paste -sd x -))"
