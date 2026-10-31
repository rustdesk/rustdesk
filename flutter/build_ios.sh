#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
flutter --version --machine | python3 -c 'import json,sys; v=json.load(sys.stdin)["frameworkVersion"]; assert v == "3.44.8", "iOS builds require Flutter 3.44.8, got " + v'
xcrun --sdk iphoneos --show-sdk-version | python3 -c 'import sys; v=sys.stdin.read().strip(); assert tuple(map(int,v.split(".")[:2])) >= (27,1), "iOS builds require SDK27.1 or later, got " + v'
ios_build_root="$PWD"
ios_build_backup="$(mktemp)"
if ! tar -cf "$ios_build_backup" flutter/lib/common.dart flutter/pubspec.yaml flutter/pubspec.lock flutter/ios/Podfile.lock \
    flutter/ios/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme; then
    rm -f "$ios_build_backup"
    exit 1
fi
trap 'tar -xf "$ios_build_backup" -C "$ios_build_root" || exit 1; rm -f "$ios_build_backup"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
bash .github/patches/apply_flutter_3.44_source_patches.sh
python3 - <<'PYTHON'
from pathlib import Path
p = Path('flutter/pubspec.yaml')
s = p.read_text()
if 'enable-swift-package-manager:' not in s:
    p.write_text(s.replace('\nflutter:\n', '\nflutter:\n  config:\n    enable-swift-package-manager: false\n', 1))
PYTHON
cd flutter
flutter build ipa --release "$@"
