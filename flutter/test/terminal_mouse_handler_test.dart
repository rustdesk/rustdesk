import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xterm/xterm.dart';

const _terminalSize = Size(400, 120);

Widget _terminalHarness(
  Terminal terminal,
  TerminalController controller,
) =>
    MaterialApp(
      home: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(
          width: _terminalSize.width,
          height: _terminalSize.height,
          child: TerminalMouseInteraction(
            terminal,
            controller: controller,
          ),
        ),
      ),
    );

void _writeLines(Terminal terminal, int count) => terminal.write(
      List.generate(count, (index) => 'line $index\r\n').join(),
    );

void main() {
  late Terminal terminal;
  late List<String> output;

  setUp(() {
    output = <String>[];
    terminal = Terminal(mouseHandler: const WheelButtonFixMouseHandler())
      ..onOutput = output.add;
  });

  testWidgets('Linux Ctrl+V is not paste', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    try {
      final messenger = tester.binding.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (_) async => {'text': 'clipboard'},
      );
      final controller = TerminalController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(_terminalHarness(terminal, controller));
      await tester.tap(find.byType(TerminalView));
      await tester.pump(kDoubleTapTimeout);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyV);
      final controlVOutput = List.of(output);
      output.clear();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyV);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      expect(controlVOutput, ['\x16']);
      expect(output, ['clipboard']);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
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

  testWidgets('dragging below scrolls and extends selection', (tester) async {
    final controller = TerminalController();
    _writeLines(terminal, 80);
    await tester.pumpWidget(_terminalHarness(terminal, controller));
    final terminalView =
        tester.state<TerminalViewState>(find.byType(TerminalView));
    final scrollController = terminalView.widget.scrollController!;
    scrollController.jumpTo(0);
    await tester.pump();
    final renderTerminal = terminalView.renderTerminal;
    const localStart = Offset(20, 20);
    final startCell = renderTerminal.getCellOffset(localStart);
    final mouse = TestPointer(1, PointerDeviceKind.mouse);
    final outside = Offset(20, renderTerminal.size.height);
    await tester.handlePointerEventRecord([
      PointerEventRecord(Duration.zero, [
        mouse.down(renderTerminal.localToGlobal(localStart)),
        mouse.move(renderTerminal.localToGlobal(outside)),
      ]),
      PointerEventRecord(const Duration(milliseconds: 150), [
        mouse.move(
          renderTerminal.localToGlobal(outside + const Offset(1, 1)),
        ),
        mouse.up(),
      ]),
    ]);

    expect(scrollController.offset, greaterThan(0));
    expect(controller.selection!.begin, startCell);
    expect(controller.selection!.end.y, greaterThan(startCell.y));
    final releasedOffset = scrollController.offset;
    await tester.pump(const Duration(milliseconds: 100));
    expect(scrollController.offset, releasedOffset);
  });

  testWidgets('tmux mouse input is reported without local selection',
      (tester) async {
    final controller = TerminalController();
    terminal.write('\x1b[?1049h\x1b[?1002h\x1b[?1006hword');
    await tester.pumpWidget(_terminalHarness(terminal, controller));
    final renderTerminal = tester
        .state<TerminalViewState>(find.byType(TerminalView))
        .renderTerminal;
    const wheel = Offset(120, 40);
    await tester.sendEventToBinding(
      PointerScrollEvent(
        position: renderTerminal.localToGlobal(wheel),
        scrollDelta: const Offset(0, 40),
      ),
    );
    await tester.pump();
    final wheelCell = renderTerminal.getCellOffset(wheel);
    expect(output.first, '\x1b[<65;${wheelCell.x + 1};${wheelCell.y + 1}M');
    output.clear();
    final clickPosition =
        renderTerminal.getOffset(const CellOffset(0, 0)) + const Offset(1, 1);
    final clickCell = renderTerminal.getCellOffset(clickPosition);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.down(renderTerminal.localToGlobal(clickPosition));
    await mouse.up();
    await tester.pump();
    await mouse.down(renderTerminal.localToGlobal(clickPosition));
    await mouse.up();
    await tester.pump(kDoubleTapTimeout);
    expect(output, [
      '\x1b[<0;${clickCell.x + 1};${clickCell.y + 1}M',
      '\x1b[<0;${clickCell.x + 1};${clickCell.y + 1}m',
      '\x1b[<0;${clickCell.x + 1};${clickCell.y + 1}M',
      '\x1b[<0;${clickCell.x + 1};${clickCell.y + 1}m',
    ]);
    expect(controller.selection, isNull);
    expect(controller.suspendedPointerInputs, isFalse);
    output.clear();
    const start = Offset(40, 40);
    const end = Offset(240, 80);
    final startCell = renderTerminal.getCellOffset(start);
    final endCell = renderTerminal.getCellOffset(end);
    await mouse.down(renderTerminal.localToGlobal(start));
    await mouse.moveTo(renderTerminal.localToGlobal(end));
    await tester.pump();

    expect(output, [
      '\x1b[<0;${startCell.x + 1};${startCell.y + 1}M',
      '\x1b[<32;${endCell.x + 1};${endCell.y + 1}M',
    ]);
    expect(controller.selection, isNull);
    await mouse.up();
    expect(output.last, '\x1b[<0;${endCell.x + 1};${endCell.y + 1}m');
    expect(controller.suspendedPointerInputs, isFalse);
  });

  testWidgets('tmux drag stays suppressed after mouse mode is disabled',
      (tester) async {
    final controller = TerminalController();
    terminal.write('\x1b[?1049h\x1b[?1002h\x1b[?1006hword');
    await tester.pumpWidget(_terminalHarness(terminal, controller));
    final renderTerminal = tester
        .state<TerminalViewState>(find.byType(TerminalView))
        .renderTerminal;
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    final start = renderTerminal.localToGlobal(const Offset(40, 40));
    final end = renderTerminal.localToGlobal(const Offset(240, 80));
    await mouse.down(start);
    output.clear();
    terminal.write('\x1b[?1002l');
    await mouse.moveTo(end);

    expect(output, isEmpty);
    expect(controller.selection, isNull);
    expect(controller.suspendedPointerInputs, isTrue);
    terminal.write('\x1b[?1002h');
    await mouse.moveTo(start);
    expect(output, isEmpty);
    expect(controller.selection, isNull);
    await mouse.up();
    expect(output, isEmpty);
    expect(controller.suspendedPointerInputs, isFalse);

    await mouse.down(start);
    output.clear();
    terminal.write('\x1b[?1002l');
    await mouse.up();
    await tester.pump(kDoubleTapTimeout);

    expect(output, isEmpty);
    expect(controller.selection, isNull);
    expect(controller.suspendedPointerInputs, isFalse);
  });
}
