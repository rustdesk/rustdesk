#!/usr/bin/env bash
# Prepares a web build on Flutter 3.44.x. Companion to
# apply_flutter_3.44_source_patches.sh (followed by 'flutter pub get'):
# the web target additionally needs qr_code_scanner's web implementation
# patched for the dart:ui platformViewRegistry removal, and
# flutter/web/fonts refreshed with the font paths the 3.44 engine requests
# for offline/air-gapped support
# (rustdesk-server-pro#996; see flutter/web/fonts/sync_fonts.py).
#
# Run from the repository root with Flutter 3.44.x on PATH, then build:
#   python3 flutter/flutter_pub_get.py check-local-pubspec-changes
#   bash .github/patches/apply_flutter_3.44_source_patches.sh
#   python3 flutter/flutter_pub_get.py get-packages "$(find flutter/ -type f -name 'pubspec-bridge-3.44.*.lock')"
#   bash .github/patches/apply_flutter_3.44_web_patches.sh
#   (cd flutter && flutter build web --release)   # or ./web/js/flutter_build.py
#   python3 flutter/flutter_pub_get.py revert-local-pubspec-changes
#
# Idempotent. To undo the source changes locally:
#   git checkout -- flutter/lib/common.dart flutter/pubspec.yaml flutter/pubspec.lock
set -euo pipefail

flutter --version | grep -q "Flutter 3\.44\." || {
  echo "Flutter 3.44.x must be on PATH; found:" >&2
  flutter --version | grep "^Flutter" >&2 || true
  exit 1
}

# qr_code_scanner 1.0.1 (unmaintained) reads platformViewRegistry from
# dart:ui, which Flutter 3.44 removed; point it at dart:ui_web instead. The
# patched file also compiles on Flutter 3.24 (dart:ui_web exists there), so
# mutating the shared pub cache is safe for other local builds.
QR_WEB="${PUB_CACHE:-$HOME/.pub-cache}/hosted/pub.dev/qr_code_scanner-1.0.1/lib/src/web/flutter_qr_web.dart"
if ! grep -qF "dart:ui_web" "$QR_WEB"; then
  sed -i.bak "s|import 'dart:ui' as ui;|import 'dart:ui' as ui; import 'dart:ui_web' as ui_web;|" "$QR_WEB"
  rm -f "$QR_WEB.bak"
fi
if grep -qF "ui.platformViewRegistry" "$QR_WEB"; then
  sed -i.bak "s|ui\.platformViewRegistry|ui_web.platformViewRegistry|g" "$QR_WEB"
  rm -f "$QR_WEB.bak"
fi

# Mirror the fonts this engine version requests into flutter/web/fonts.
python3 flutter/web/fonts/sync_fonts.py

# Fail loudly if any expected state is missing:
grep -qF "import 'dart:ui' as ui; import 'dart:ui_web' as ui_web;" "$QR_WEB"
grep -qF "ui_web.platformViewRegistry" "$QR_WEB"
grep -qF 'google_fonts: ^8.1.0' flutter/pubspec.yaml

echo "Flutter 3.44 web patches applied."
