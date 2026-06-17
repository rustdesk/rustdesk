import 'dart:ffi' show Abi;
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Font family name registered with [FontLoader] when a system CJK font is
/// successfully loaded on ARM64 Linux.
const kLinuxCjkFontFamily = 'SystemCJK';

const _kFontSearchPaths = [
  // Debian / Ubuntu (noto-fonts / fonts-noto-cjk)
  '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc',
  '/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc',
  '/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf',
  // Fedora / RHEL / Rocky (google-noto-sans-cjk-fonts)
  '/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc',
  '/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc',
  // Arch Linux (noto-fonts-cjk)
  '/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc',
  '/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf',
  // Generic fallback paths
  '/usr/share/fonts/noto/NotoSansCJK-Regular.ttc',
  '/usr/share/fonts/noto/NotoSansCJKsc-Regular.otf',
  // WenQuanYi — commonly pre-installed on CJK-locale systems
  '/usr/share/fonts/truetype/wqy/wqy-microhei.ttc',
  '/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc',
  '/usr/share/fonts/wqy-microhei/wqy-microhei.ttc',
  '/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc',
];

/// Loads a system CJK font on ARM64 Linux into Flutter's font registry via
/// [FontLoader], working around the missing fontconfig support in the
/// flutter-elinux engine (https://github.com/flutter/flutter/issues/139293).
///
/// Returns true if a CJK font was successfully loaded; false otherwise.
/// On all other platforms this is a no-op and returns false immediately.
Future<bool> loadSystemCJKFonts() async {
  if (Abi.current() != Abi.linuxArm64) return false;

  final path = await _findCjkFontPath();
  if (path == null) {
    debugPrint('ARM64 Linux: no CJK font found; CJK text may not render');
    return false;
  }

  try {
    final loader = FontLoader(kLinuxCjkFontFamily);
    final bytes = await File(path).readAsBytes();
    loader.addFont(Future.value(ByteData.view(bytes.buffer, bytes.offsetInBytes, bytes.lengthInBytes)));
    await loader.load();
    debugPrint('ARM64 Linux: loaded CJK font from $path');
    return true;
  } catch (e) {
    debugPrint('ARM64 Linux: failed to load CJK font: $e');
    return false;
  }
}

Future<String?> _findCjkFontPath() async {
  // Query fc-list for each CJK script separately.  Fonts present in all three
  // sets (zh ∩ ja ∩ ko) are true pan-CJK fonts; prefer them so we don't
  // accidentally pick a Chinese-only font that lacks Japanese kana or Korean
  // hangul glyphs.  fc-list is a fontconfig CLI tool available on most Linux
  // systems independent of whether the Flutter engine was built with fontconfig.
  final byLang = <String, Set<String>>{};
  for (final lang in const ['zh', 'ja', 'ko']) {
    final paths = <String>{};
    try {
      final r =
          await Process.run('fc-list', [':lang=$lang', '--format=%{file}\n']);
      if (r.exitCode == 0) {
        for (final line in r.stdout.toString().split('\n')) {
          final p = line.trim();
          if (p.isNotEmpty && File(p).existsSync()) paths.add(p);
        }
      }
    } catch (_) {}
    byLang[lang] = paths;
  }

  final panCjk = byLang['zh']!
      .intersection(byLang['ja']!)
      .intersection(byLang['ko']!);
  final anyCjk =
      byLang.values.fold(<String>{}, (acc, s) => acc..addAll(s));

  // Among candidates, prefer well-known pan-CJK font families.
  String? pick(Iterable<String> pool) {
    const preferred = ['notosanscjk', 'sourcehansans', 'sourcehanserif'];
    for (final name in preferred) {
      for (final p in pool) {
        if (p.toLowerCase().contains(name)) return p;
      }
    }
    return pool.isNotEmpty ? pool.first : null;
  }

  final found = pick(panCjk) ?? pick(anyCjk);
  if (found != null) return found;

  for (final p in _kFontSearchPaths) {
    if (File(p).existsSync()) return p;
  }
  return null;
}
