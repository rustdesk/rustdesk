#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
flutter --version --machine | python3 -c 'import json,sys; v=json.load(sys.stdin)["frameworkVersion"]; assert v == "3.44.8", "iOS builds require Flutter 3.44.8, got " + v'
xcrun --sdk iphoneos --show-sdk-version | python3 -c 'import sys; v=sys.stdin.read().strip(); assert tuple(map(int,v.split(".")[:2])) >= (27,1), "iOS builds require SDK27.1 or later, got " + v'
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
