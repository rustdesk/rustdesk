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
