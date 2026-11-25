#!/usr/bin/env bash
set -euo pipefail

sdk="$RUNNER_TEMP/flutter-preview"
git clone --depth 1 --branch 3.50.1 https://github.com/flutter/flutter.git "$sdk"
revision=$(git -C "$sdk" rev-parse HEAD)
if [[ "$revision" != ae9b586f34a8443573f82ef57c3bf914a908cbf7 ]]; then
  echo "Unexpected Flutter revision: $revision" >&2
  exit 1
fi
echo "$sdk/bin" >> "$GITHUB_PATH"
