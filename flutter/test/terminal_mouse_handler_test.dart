import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xterm/xterm.dart';

void main() {
  late Terminal terminal;
  late List<String> output;

  setUp(() {
    output = <String>[];
    terminal = Terminal(mouseHandler: const WheelButtonFixMouseHandler())
      ..onOutput = output.add;
  });

  String? report(
    TerminalMouseButton button, [
    TerminalMouseButtonState state = TerminalMouseButtonState.down,
    CellOffset position = const CellOffset(10, 5),
  ]) {
    output.clear();
    terminal.mouseInput(button, state, position);
    return output.isEmpty ? null : output.single;
  }

  test('reports SGR wheel buttons without the Shift modifier bit', () {
    terminal.write('\x1b[?1000h\x1b[?1006h');

    expect(report(TerminalMouseButton.wheelUp), '\x1b[<64;11;6M');
    expect(report(TerminalMouseButton.wheelDown), '\x1b[<65;11;6M');
    expect(report(TerminalMouseButton.wheelLeft), '\x1b[<66;11;6M');
    expect(report(TerminalMouseButton.wheelRight), '\x1b[<67;11;6M');
  });

  test('reports normal-encoding wheel buttons in the 64..67 range', () {
    terminal.write('\x1b[?1000h');

    expect(
      report(TerminalMouseButton.wheelUp),
      '\x1b[M${String.fromCharCode(32 + 64)}'
      '${String.fromCharCode(32 + 11)}${String.fromCharCode(32 + 6)}',
    );
    expect(
      report(TerminalMouseButton.wheelDown),
      '\x1b[M${String.fromCharCode(32 + 65)}'
      '${String.fromCharCode(32 + 11)}${String.fromCharCode(32 + 6)}',
    );
  });

  test('reports utf-encoding wheel buttons beyond the normal-mode range', () {
    terminal.write('\x1b[?1000h\x1b[?1005h');

    expect(
      report(
        TerminalMouseButton.wheelDown,
        TerminalMouseButtonState.down,
        const CellOffset(400, 300),
      ),
      '\x1b[M${String.fromCharCode(32 + 65)}'
      '${String.fromCharCode(32 + 401)}${String.fromCharCode(32 + 301)}',
    );
  });

  test('reports urxvt-encoding wheel buttons shifted by 32', () {
    terminal.write('\x1b[?1000h\x1b[?1015h');

    expect(report(TerminalMouseButton.wheelUp), '\x1b[96;11;6M');
    expect(report(TerminalMouseButton.wheelDown), '\x1b[97;11;6M');
  });

  test('sends a null byte for coordinates past the encoding limit', () {
    terminal.write('\x1b[?1000h');

    expect(
      report(
        TerminalMouseButton.wheelUp,
        TerminalMouseButtonState.down,
        const CellOffset(300, 300),
      ),
      '\x1b[M${String.fromCharCode(32 + 64)}\x00\x00',
    );
  });

  test('leaves non-wheel buttons to the upstream handler', () {
    terminal.write('\x1b[?1000h\x1b[?1006h');

    expect(report(TerminalMouseButton.left), '\x1b[<0;11;6M');
    expect(report(TerminalMouseButton.middle), '\x1b[<1;11;6M');
    expect(
      report(TerminalMouseButton.right, TerminalMouseButtonState.up),
      '\x1b[<2;11;6m',
    );
  });

  test('stays silent when the peer has not enabled mouse reporting', () {
    expect(report(TerminalMouseButton.wheelDown), isNull);
    expect(report(TerminalMouseButton.left), isNull);
  });

  test('stays silent for the wheel in click-only mode', () {
    terminal.write('\x1b[?9h\x1b[?1006h');

    expect(report(TerminalMouseButton.wheelDown), isNull);
    expect(report(TerminalMouseButton.left), '\x1b[<0;11;6M');
  });

  test('does not report wheel button releases', () {
    terminal.write('\x1b[?1000h\x1b[?1006h');

    expect(
      report(TerminalMouseButton.wheelDown, TerminalMouseButtonState.up),
      isNull,
    );
  });
}
