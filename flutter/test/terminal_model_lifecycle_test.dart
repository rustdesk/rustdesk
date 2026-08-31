import 'dart:async';

import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xterm/xterm.dart';

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

  test('ignores terminal text input after the terminal model is disposed', () {
    final model = TerminalModel(_FakeFFI());
    var checkedCtrlLock = false;
    var clearedCtrlLock = false;

    model.isCtrlLocked = () {
      checkedCtrlLock = true;
      return true;
    };
    model.clearCtrlLock = () {
      clearedCtrlLock = true;
    };

    model.dispose();
    model.terminal.textInput('d');

    expect(checkedCtrlLock, isFalse);
    expect(clearedCtrlLock, isFalse);
    expect(model.debugBufferedInputCount, 0);
  });

  test('builds its terminal with the wheel button fix', () {
    final model = TerminalModel(_FakeFFI());
    addTearDown(model.dispose);

    final captured = <String>[];
    model.terminal.onOutput = captured.add;
    model.terminal.write('\x1b[?1000h\x1b[?1006h');
    model.terminal.mouseInput(
      TerminalMouseButton.wheelUp,
      TerminalMouseButtonState.down,
      const CellOffset(10, 5),
    );

    expect(captured.single, '\x1b[<64;11;6M');
  });
}
