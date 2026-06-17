/// Web stub for `native/font_manager.dart`.
///
/// The native implementation depends on `dart:io` (Process/File/Platform) to
/// load a system CJK font on ARM64 Linux, which cannot compile for the web
/// target. The web build has no such fontconfig limitation, so this is a no-op.
const kLinuxCjkFontFamily = 'SystemCJK';

Future<bool> loadSystemCJKFonts() async => false;
