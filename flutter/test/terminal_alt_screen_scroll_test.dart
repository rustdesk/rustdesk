import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xterm/xterm.dart';

const _captureMouse = '\x1b[?1007l\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1003h';

void main() {
  late Terminal terminal;
  late TerminalController controller;
  late List<String> output;

  setUp(() {
    output = [];
    terminal = Terminal()..onOutput = output.add;
    controller = TerminalController();
  });

  Future<TerminalViewState> mount(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: Align(
        alignment: Alignment.topLeft,
        child: Padding(
          padding: const EdgeInsets.only(left: 30, top: 50),
          child: SizedBox(
            width: 400,
            height: 120,
            child: TerminalMouseInteraction(terminal, controller: controller),
          ),
        ),
      ),
    ));
    terminal.write(List.generate(80, (i) => 'line $i\r\n').join());
    await tester.pump();
    final view = tester.state<TerminalViewState>(find.byType(TerminalView));
    expect(view.widget.scrollController!.offset, greaterThan(0));
    return view;
  }

  Future<void> scrollTrackpad(
      WidgetTester tester, Offset point, double distance) async {
    final pointer = TestPointer(2, PointerDeviceKind.trackpad);
    await tester.sendEventToBinding(pointer.panZoomStart(point));
    await tester.sendEventToBinding(
        pointer.panZoomUpdate(point, pan: Offset(0, distance)));
    await tester.sendEventToBinding(
        pointer.panZoomUpdate(point, pan: Offset(0, distance * 2)));
    await tester.sendEventToBinding(pointer.panZoomEnd());
    await tester.pump();
  }

  for (final trackpad in [false, true]) {
    final device = trackpad ? 'trackpad' : 'wheel';
    testWidgets('alternate screen reports $device after main-screen history',
        (tester) async {
      final view = await mount(tester);
      terminal.write('\x1b[?1049h$_captureMouse');
      await tester.pump();
      final render = view.renderTerminal;
      final point = render.localToGlobal(
          Offset(render.cellSize.width * 2.5, render.lineHeight * 1.5));
      if (trackpad) {
        await scrollTrackpad(tester, point, render.lineHeight * 3);
      } else {
        await tester.sendEventToBinding(PointerScrollEvent(
          position: point,
          scrollDelta: Offset(0, -render.lineHeight * 3),
        ));
        await tester.pump();
      }

      expect(output, isNotEmpty);
      expect(output, everyElement('\x1b[<64;3;2M'));
      expect(view.widget.scrollController!.offset, 0);
      expect(controller.selection, isNull);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  }

  testWidgets('trackpad scrolling keeps subsequent drag coordinates aligned',
      (tester) async {
    final view = await mount(tester);
    terminal.write('\x1b[?1049h$_captureMouse');
    await tester.pump();
    final render = view.renderTerminal;
    final start = render.localToGlobal(
        Offset(render.cellSize.width * 2.5, render.lineHeight * 1.5));
    final end = render.localToGlobal(
        Offset(render.cellSize.width * 6.5, render.lineHeight * 2.5));
    await scrollTrackpad(tester, start, render.lineHeight * 3);
    output.clear();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.down(start);
    await mouse.moveTo(end);
    await mouse.up();
    await tester.pump(kDoubleTapTimeout);

    expect(output, ['\x1b[<0;3;2M', '\x1b[<32;7;3M', '\x1b[<0;7;3m']);
    expect(controller.selection, isNull);
    expect(controller.suspendedPointerInputs, isFalse);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('leaving alternate screen restores local trackpad scrollback',
      (tester) async {
    final view = await mount(tester);
    terminal.write('\x1b[?1049h$_captureMouse');
    await tester.pump();
    terminal.write('\x1b[?1003l\x1b[?1002l\x1b[?1000l\x1b[?1006l\x1b[?1049l');
    await tester.pump();
    final before = view.widget.scrollController!.offset;
    await scrollTrackpad(
        tester, view.renderTerminal.localToGlobal(const Offset(40, 40)), 48);

    expect(output, isEmpty);
    expect(view.widget.scrollController!.offset, lessThan(before));
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('alternate screen without mouse capture still simulates arrows',
      (tester) async {
    final view = await mount(tester);
    terminal.write('\x1b[?1049h');
    await tester.pump();
    await scrollTrackpad(
        tester, view.renderTerminal.localToGlobal(const Offset(40, 40)), -48);

    expect(output, isNotEmpty);
    expect(output, everyElement('\x1b[B'));
    expect(view.widget.scrollController!.offset, 0);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
