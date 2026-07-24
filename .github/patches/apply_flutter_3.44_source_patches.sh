#!/usr/bin/env bash
# Applies the Flutter 3.44-only source/pubspec changes on the fly, in CI only.
#
# Windows arm64 needs Flutter >= 3.44 (the first stable release shipping an arm64 Dart SDK +
# engine), and macOS needs a newer engine to avoid flutter/flutter#148279. Flutter 3.44 renamed
# DialogTheme/TabBarTheme -> *Data and needs newer extended_text/google_fonts. Other platforms
# are still on Flutter 3.24.5, so these changes stay OUT of committed sources and are applied here.
#
# Used by the Windows arm64 and macOS builds (flutter-build.yml) and their dedicated bridge
# artifact (bridge.yml) so they share an identical 3.44 source state -- generated *.freezed.dart
# must compile against the same Flutter/freezed version those builds resolve.
#
# Remove this script (and commit the changes) once upstream bumps Flutter across the board.
#
# Run from the repository root. sed is used (not a git-apply patch) because the checked-out
# sources are CRLF on the windows-11-arm runner; the substitutions below are anchor-free and
# therefore CRLF-safe.
set -euo pipefail

sed_in_place() {
  sed -i.bak "$@"
}

# ThemeData API renames (Flutter 3.27+):
sed_in_place 's/dialogTheme: DialogTheme(/dialogTheme: DialogThemeData(/g' flutter/lib/common.dart
sed_in_place 's/tabBarTheme: const TabBarTheme(/tabBarTheme: const TabBarThemeData(/g' flutter/lib/common.dart
sed_in_place '/static ThemeData lightTheme = ThemeData(/,/static ThemeData darkTheme = ThemeData(/s/dialogTheme: DialogThemeData(/dialogTheme: DialogThemeData(\
      backgroundColor: Colors.white,/' flutter/lib/common.dart
sed_in_place '/static ThemeData darkTheme = ThemeData(/,/scrollbarTheme: scrollbarThemeDark,/s/dialogTheme: DialogThemeData(/dialogTheme: DialogThemeData(\
      backgroundColor: Color(0xFF18191E),/' flutter/lib/common.dart
# Dependency bumps required by the newer Dart/Flutter:
sed_in_place 's/extended_text: 14.0.0/extended_text: 15.0.2/' flutter/pubspec.yaml
sed_in_place 's/google_fonts: \^6.2.1/google_fonts: ^8.1.0/' flutter/pubspec.yaml
rm -f flutter/lib/common.dart.bak flutter/pubspec.yaml.bak

# Fail loudly if any expected string drifted, so we never silently build unpatched:
grep -qF 'dialogTheme: DialogThemeData(' flutter/lib/common.dart
grep -qF 'tabBarTheme: const TabBarThemeData(' flutter/lib/common.dart
grep -qF 'backgroundColor: Colors.white,' flutter/lib/common.dart
grep -qF 'backgroundColor: Color(0xFF18191E),' flutter/lib/common.dart
grep -qF 'extended_text: 15.0.2' flutter/pubspec.yaml
grep -qF 'google_fonts: ^8.1.0' flutter/pubspec.yaml

git --no-pager diff -- flutter/lib/common.dart flutter/pubspec.yaml
