#!/usr/bin/env bash
#
# Regenerate the Alfred workflow icon (workflow/icon.png) from the Datadog logo
# published as the avatar of the DataDog GitHub organization. The logo is used
# unmodified; Datadog is a trademark of Datadog, Inc.
#
# Requires: curl, sips (ships with macOS).
#
# Usage: scripts/icon.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SOURCE="https://github.com/DataDog.png?size=512"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL -o "$TMP/logo.png" "$SOURCE"
sips -s format png "$TMP/logo.png" --out workflow/icon.png >/dev/null

echo "==> Wrote workflow/icon.png ($(sips -g pixelWidth -g pixelHeight workflow/icon.png | awk '/pixel/{print $2}' | paste -sd x -))"
