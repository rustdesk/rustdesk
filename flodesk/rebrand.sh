#!/usr/bin/env bash
# usage: ./rebrand.sh <rustdesk-checkout> <logo.png 512+> [rendezvous-host] [pubkey]
# Needs: ImageMagick (convert), git submodules initialised (libs/hbb_common).
set -euo pipefail
RD=$(realpath "$1"); LOGO=$(realpath "$2"); HOST=${3:-}; KEY=${4:-}
cd "$RD"
sedi() { sed -i "$@"; }

# --- 0) preflight: logo + real ImageMagick (Windows also ships an unrelated System32\convert.exe) ---
[ -f "$LOGO" ] || { echo "FATAL: logo not found: $LOGO" >&2; exit 1; }
IM=()
if command -v magick >/dev/null 2>&1 && magick --version 2>/dev/null | grep -qi imagemagick; then IM=(magick)
elif command -v convert >/dev/null 2>&1 && convert --version 2>/dev/null | grep -qi imagemagick; then IM=(convert)
else echo "FATAL: ImageMagick not found (magick/convert; Windows convert.exe is not ImageMagick)" >&2; exit 1; fi
dims=$("${IM[@]}" identify -format '%w %h' "$LOGO" 2>/dev/null || identify -format '%w %h' "$LOGO")
set -- $dims; { [ "${1:-0}" -ge 512 ] && [ "${2:-0}" -ge 512 ]; } || { echo "FATAL: logo must be >=512x512 (got ${1:-?}x${2:-?})" >&2; exit 1; }
echo "ImageMagick: ${IM[*]}; logo ${1}x${2}"

# --- 1) names / ids (verified against upstream layout) ---
sedi 's/RwLock::new("RustDesk"/RwLock::new("FloDesk"/' libs/hbb_common/src/config.rs
sedi -e 's/^ProductName = .*/ProductName = "FloDesk"/' \
     -e 's/^FileDescription = .*/FileDescription = "FloDesk Remote Desktop"/' \
     -e 's/^OriginalFilename = .*/OriginalFilename = "FloDesk.exe"/' Cargo.toml
sedi -e 's/"RustDesk Remote Desktop"/"FloDesk Remote Desktop"/' \
     -e 's/"InternalName", "rustdesk"/"InternalName", "FloDesk"/' \
     -e 's/"rustdesk.exe"/"FloDesk.exe"/' \
     -e 's/"ProductName", "RustDesk"/"ProductName", "FloDesk"/' flutter/windows/runner/Runner.rc
sedi 's/L"RustDesk"/L"FloDesk"/' flutter/windows/runner/main.cpp
# final exe name: build.py renames rustdesk.exe -> RustDesk.exe at the portable step
sedi 's#target/release/RustDesk.exe#target/release/FloDesk.exe#' build.py
# Android: ONLY applicationId (namespace + JNI/Kotlin package must stay)
sedi 's/applicationId "com.carriez.flutter_hbb"/applicationId "top.poshakerp.flodesk"/' flutter/android/app/build.gradle
sedi -e 's/android:label="RustDesk"/android:label="فلو دسک"/' \
     -e 's/android:label="RustDesk Input"/android:label="FloDesk Input"/' \
     flutter/android/app/src/main/AndroidManifest.xml

# --- 2) icons ---
"${IM[@]}" "$LOGO" -define icon:auto-resize=256,128,64,48,32,16 res/icon.ico
cp res/icon.ico flutter/windows/runner/resources/app_icon.ico
"${IM[@]}" "$LOGO" -resize 512x512 res/icon.png
for s in 32 64 128; do "${IM[@]}" "$LOGO" -resize ${s}x${s} res/${s}x${s}.png; done
"${IM[@]}" "$LOGO" -resize 256x256 res/128x128@2x.png
R=flutter/android/app/src/main/res
declare -A L=([mdpi]=48 [hdpi]=72 [xhdpi]=96 [xxhdpi]=144 [xxxhdpi]=192)
for d in "${!L[@]}"; do
  s=${L[$d]}; f=$((s*9/4)); in=$((f*60/100))   # adaptive: 108dp canvas, logo in safe zone
  "${IM[@]}" "$LOGO" -resize ${s}x${s} $R/mipmap-$d/ic_launcher.png
  "${IM[@]}" "$LOGO" -resize ${s}x${s} $R/mipmap-$d/ic_launcher_round.png
  "${IM[@]}" -size ${f}x${f} xc:none \( "$LOGO" -resize ${in}x${in} \) -gravity center -composite $R/mipmap-$d/ic_launcher_foreground.png
done
sedi 's#>\#[0-9A-Fa-f]\{6,8\}<#>\#0F1115<#' $R/values/colors.xml 2>/dev/null || true
mkdir -p flutter/assets && cp "$LOGO" flutter/assets/flodesk_logo.png
grep -q flodesk_logo flutter/pubspec.yaml || echo "TODO: add 'assets/flodesk_logo.png' under flutter.assets in pubspec.yaml if not covered by a directory entry"

# --- 3) theme: black & white, RTL, Vazirmatn ---
T=flutter/lib/common.dart
sedi -e 's/static const Color accent = Color(0xFF0071FF)/static const Color accent = Color(0xFFFFFFFF)/' \
     -e 's/static const Color button = Color(0xFF2C8CFF)/static const Color button = Color(0xFF2A2F3A)/' \
     -e 's/static const Color idColor = Color(0xFF00B6F0)/static const Color idColor = Color(0xFFFFFFFF)/' \
     -e 's/scaffoldBackgroundColor: Color(0xFF18191E)/scaffoldBackgroundColor: Color(0xFF0F1115)/' \
     -e 's/static const Color canvasColor = Color(0xFF212121)/static const Color canvasColor = Color(0xFF1F232B)/' \
     -e 's/static const Color border = Color(0xFFCCCCCC)/static const Color border = Color(0xFFE5E4E2)/' $T
mkdir -p flutter/assets/fonts
# Vazirmatn: download the OFL font files into flutter/assets/fonts/ (see README), then:
python3 - <<'PY'
import re,io
p='flutter/pubspec.yaml'; s=open(p,encoding='utf8').read()
if 'Vazirmatn' not in s:
    s=s.replace("  fonts:\n","  fonts:\n    - family: Vazirmatn\n      fonts:\n        - asset: assets/fonts/Vazirmatn-Regular.ttf\n        - asset: assets/fonts/Vazirmatn-Bold.ttf\n          weight: 700\n",1)
    open(p,'w',encoding='utf8').write(s)
PY
sedi -E "s/(static ThemeData (light|dark)Theme = ThemeData\()/\1\n    fontFamily: 'Vazirmatn',/" $T
echo "RTL: set locale fa in flutter/lib/main.dart (Locale('fa')) or select فارسی in app language; ID/password card font: edit desktop_home_page.dart buildIDBoard/buildPasswordBoard TextStyle(fontSize: 32+)."

# --- 4) default server (optional) ---
if [ -n "$HOST" ]; then
  sedi "s#RENDEZVOUS_SERVERS: &\[&str\] = &\[\".*\"\]#RENDEZVOUS_SERVERS: \&[\&str] = \&[\"$HOST\"]#" libs/hbb_common/src/config.rs
fi
[ -n "$KEY" ] && sedi "s#RS_PUB_KEY: &str = \".*\"#RS_PUB_KEY: \&str = \"$KEY\"#" libs/hbb_common/src/config.rs
git diff --stat | tail -20
