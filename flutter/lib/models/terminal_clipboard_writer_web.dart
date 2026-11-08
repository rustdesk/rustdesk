import 'dart:js_interop';

import 'package:flutter/foundation.dart';

const _writeTerminalClipboardCommand = 'write_terminal_clipboard';

@JS('setByName')
external JSPromise<JSBoolean> _setByName(
  JSString name,
  JSString value,
  JSBoolean userInitiated,
);

Future<bool> writeTerminalClipboardPlatform(
  String text, {
  bool userInitiated = false,
}) async {
  try {
    final result = await _setByName(
      _writeTerminalClipboardCommand.toJS,
      text.toJS,
      userInitiated.toJS,
    ).toDart;
    return result.toDart;
  } catch (error) {
    debugPrint('[Terminal] Failed to write Web clipboard: $error');
    return false;
  }
}
