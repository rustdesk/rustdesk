import 'dart:async';

import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _FakeFFI implements FFI {
  @override
  String id = 'test-peer';

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  test('ignores paste that completes after the terminal model is disposed',
      () async {
    final model = TerminalModel(_FakeFFI());
    final delayedClipboardText = Completer<String>();

    // This mirrors Ctrl/Cmd+V: clipboard access starts first, then the page and
    // model are disposed before the asynchronous read supplies its text.
    final paste = delayedClipboardText.future.then(model.pasteText);
    model.dispose();
    delayedClipboardText.complete('late clipboard text');
    await paste;

    expect(model.debugBufferedInputCount, 0);
  });
}
