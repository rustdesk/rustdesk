#!/usr/bin/env bash
# Applies the Flutter 3.44-only source/pubspec changes on the fly, in CI only.
#
# Windows arm64 needs Flutter >= 3.44 (the first stable release shipping an arm64 Dart SDK +
# engine), which renamed DialogTheme/TabBarTheme -> *Data and needs newer extended_text/
# google_fonts. Every other platform is still on Flutter 3.24.5, where the old names/versions
# are required, so these changes are kept OUT of the committed sources and applied here instead.
#
# Used by BOTH the Windows arm64 build (flutter-build.yml) and its dedicated bridge artifact
# (bridge.yml) so they share an identical 3.44 source state -- the generated *.freezed.dart must
# compile against the same Flutter/freezed version the arm64 build resolves.
#
# Remove this script (and commit the changes) once upstream bumps Flutter across the board.
#
# Run from the repository root. sed is used (not a git-apply patch) because the checked-out
# sources are CRLF on the windows-11-arm runner; the substitutions below are anchor-free and
# therefore CRLF-safe.
set -euo pipefail

readonly NO_MATCHES=0
readonly SINGLE_MATCH=1
readonly THEME_MATCHES=2

has_exact_count() {
  local -r expected_count="$1"
  local -r pattern="$2"
  local -r file="$3"
  local actual_count
  [[ -r "$file" ]] || return 1
  actual_count="$(grep -cF "$pattern" "$file" || true)"
  [[ "$actual_count" -eq "$expected_count" ]]
}

# The target background-color line must directly follow DialogThemeData in the selected range.
has_dialog_background_in_theme_range() {
  local -r start_pattern="$1"
  local -r end_pattern="$2"
  local -r target_pattern="$3"
  local -r file="$4"
  awk -v start_pattern="$start_pattern" \
    -v end_pattern="$end_pattern" \
    -v target_pattern="$target_pattern" '
      index($0, start_pattern) {
        in_theme = 1
        next
      }
      in_theme && index($0, end_pattern) {
        exit
      }
      in_theme && index($0, "dialogTheme: DialogThemeData(") {
        if (getline > 0) {
          line = $0
          sub(/\r$/, "", line)
          sub(/^[[:space:]]+/, "", line)
          matched = line == target_pattern
        }
        exit
      }
      END {
        exit matched ? 0 : 1
      }
    ' "$file"
}

validate_patch_inputs() {
  if [[ ! -f flutter/lib/common.dart || ! -r flutter/lib/common.dart ]]; then
    echo "Flutter 3.44 source patch input is missing or unreadable: flutter/lib/common.dart" >&2
    return 1
  fi
  if [[ ! -f flutter/pubspec.yaml || ! -r flutter/pubspec.yaml ]]; then
    echo "Flutter 3.44 source patch input is missing or unreadable: flutter/pubspec.yaml" >&2
    return 1
  fi
}

is_complete_patch_state() {
  has_exact_count "$THEME_MATCHES" 'dialogTheme: DialogThemeData(' flutter/lib/common.dart &&
    has_exact_count "$THEME_MATCHES" 'tabBarTheme: const TabBarThemeData(' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'backgroundColor: Colors.white,' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'backgroundColor: Color(0xFF18191E),' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'extended_text: 15.0.2' flutter/pubspec.yaml &&
    has_exact_count "$SINGLE_MATCH" 'google_fonts: ^8.1.0' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'dialogTheme: DialogTheme(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'tabBarTheme: const TabBarTheme(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'extended_text: 14.0.0' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'google_fonts: ^6.2.1' flutter/pubspec.yaml &&
    has_dialog_background_in_theme_range 'static ThemeData lightTheme = ThemeData(' \
      'static ThemeData darkTheme = ThemeData(' 'backgroundColor: Colors.white,' \
      flutter/lib/common.dart &&
    has_dialog_background_in_theme_range 'static ThemeData darkTheme = ThemeData(' \
      'scrollbarTheme: scrollbarThemeDark,' 'backgroundColor: Color(0xFF18191E),' \
      flutter/lib/common.dart
}

is_unpatched_state() {
  has_exact_count "$THEME_MATCHES" 'dialogTheme: DialogTheme(' flutter/lib/common.dart &&
    has_exact_count "$THEME_MATCHES" 'tabBarTheme: const TabBarTheme(' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'extended_text: 14.0.0' flutter/pubspec.yaml &&
    has_exact_count "$SINGLE_MATCH" 'google_fonts: ^6.2.1' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'dialogTheme: DialogThemeData(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'tabBarTheme: const TabBarThemeData(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'backgroundColor: Colors.white,' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'backgroundColor: Color(0xFF18191E),' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'extended_text: 15.0.2' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'google_fonts: ^8.1.0' flutter/pubspec.yaml
}

if ! validate_patch_inputs; then
  exit 1
fi

if is_complete_patch_state; then
  echo "Flutter 3.44 source patches already applied."
  git --no-pager diff -- flutter/lib/common.dart flutter/pubspec.yaml
  exit 0
fi

if ! is_unpatched_state; then
  echo "Flutter 3.44 source patches are partially applied or their anchors have drifted." >&2
  exit 1
fi

# ThemeData API renames (Flutter 3.27+):
sed -i 's/dialogTheme: DialogTheme(/dialogTheme: DialogThemeData(/g' flutter/lib/common.dart
sed -i 's/tabBarTheme: const TabBarTheme(/tabBarTheme: const TabBarThemeData(/g' flutter/lib/common.dart
sed -i '/static ThemeData lightTheme = ThemeData(/,/static ThemeData darkTheme = ThemeData(/s/dialogTheme: DialogThemeData(/dialogTheme: DialogThemeData(\
      backgroundColor: Colors.white,/' flutter/lib/common.dart
sed -i '/static ThemeData darkTheme = ThemeData(/,/scrollbarTheme: scrollbarThemeDark,/s/dialogTheme: DialogThemeData(/dialogTheme: DialogThemeData(\
      backgroundColor: Color(0xFF18191E),/' flutter/lib/common.dart
# Dependency bumps required by the newer Dart/Flutter:
sed -i 's/extended_text: 14.0.0/extended_text: 15.0.2/' flutter/pubspec.yaml
sed -i 's/google_fonts: \^6.2.1/google_fonts: ^8.1.0/' flutter/pubspec.yaml

# Fail loudly if any expected substitution did not produce the complete state.
if ! is_complete_patch_state; then
  echo "Flutter 3.44 source patches did not produce the expected state." >&2
  exit 1
fi

git --no-pager diff -- flutter/lib/common.dart flutter/pubspec.yaml
