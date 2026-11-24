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
