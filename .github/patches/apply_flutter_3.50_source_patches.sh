#!/usr/bin/env bash
set -euo pipefail

bash .github/patches/apply_flutter_3.44_source_patches.sh

# 2.3.0 reads const Row/Column instance fields in fallback expressions.
# Dart 3.14 rejects them during kernel compilation. Both RustDesk callers
# use the horizontal ToggleSwitch API already available in 2.1.0.
file=flutter/pubspec.yaml
if grep -qF 'toggle_switch: ^2.1.0' "$file"; then
  sed -i 's/toggle_switch: \^2.1.0/toggle_switch: 2.1.0/' "$file"
fi
grep -qF 'toggle_switch: 2.1.0' "$file"

# Compile the official Window API only in builds pinned to the preview SDK.
# The normal package continues to resolve the upstream plugin.
if ! grep -qF 'path: ../examples/flutter_windowing' "$file"; then
  grep -qF 'ref: cddc2e64f8fefce4f80db87b20bcda2dd019bed6' "$file"
  sed -i '/^  desktop_multi_window:[[:space:]]*$/,+3c\  desktop_multi_window:\n    path: ../examples/flutter_windowing' "$file"
fi
grep -qF 'path: ../examples/flutter_windowing' "$file"
cp examples/flutter_windowing/rustdesk_preview.dart flutter/lib/desktop/windowing_preview.dart
