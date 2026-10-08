import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

Future<bool> writeTerminalClipboardPlatform(
  String text, {
  bool userInitiated = false,
}) async {
  try {
    await Clipboard.setData(ClipboardData(text: text));
    return true;
  } catch (error) {
    debugPrint('[Terminal] Failed to write clipboard: $error');
    return false;
  }
}
