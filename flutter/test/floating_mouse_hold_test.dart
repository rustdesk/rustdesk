@TestOn('browser')

import 'dart:async';
import 'dart:js' as js;

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Input extends Fake implements InputModel {
  final events = <(String, MouseButtons)>[];

  @override
  Future<void> tapDown(MouseButtons button) async =>
      events.add(('down', button));

  @override
  Future<void> tapUp(MouseButtons button) async => events.add(('up', button));
}

class _FFI extends Fake implements FFI {}

class _Cursor extends CursorModel {
  _Cursor(FFI ffi) : super(WeakReference(ffi));
  final syncs = <Completer<void>>[];

  @override
  Future<void> syncCursorPosition() {
    final sync = Completer<void>();
    syncs.add(sync);
    return sync.future;
  }
}

void main() {
  setUp(() {
    isSpecialHoldDragActive = false;
    js.context['isMobile'] = js.allowInterop(() => true);
    js.context['getByName'] =
        js.allowInterop((String name, [String? key]) => '');
    js.context['setByName'] = js.allowInterop((String name, dynamic value) {});
  });
  tearDown(() {
    isSpecialHoldDragActive = false;
    for (final name in ['isMobile', 'getByName', 'setByName']) {
      js.context.deleteProperty(name);
    }
  });

  for (final end in ['release', 'cancel', 'dispose']) {
    testWidgets('pending hold cannot press after $end', (tester) async {
      final cancel = end == 'cancel';
      final ffi = _FFI();
      final input = _Input();
      final cursor = _Cursor(ffi);
      final button = cancel ? MouseButtons.right : MouseButtons.left;
      await tester.pumpWidget(MaterialApp(
        home: Stack(children: [
          FloatingLeftRightButton(
              isLeft: !cancel, inputModel: input, cursorModel: cursor),
        ]),
      ));
      await tester.pump();
      final target = find.byType(FloatingLeftRightButton);
      final first = await tester.startGesture(tester.getCenter(target));
      await tester.pump(const Duration(milliseconds: 201));
      expect(cursor.syncs, hasLength(1));
      if (end == 'dispose') {
        await tester.pumpWidget(const SizedBox());
        cursor.syncs.single.complete();
        await tester.pump(const Duration(milliseconds: 1));
        expect(input.events, isEmpty);
        return;
      }
      if (cancel) {
        await first.cancel();
      } else {
        await first.up();
      }
      // A new press must not make the old synchronization valid again.
      final second = await tester.startGesture(tester.getCenter(target));
      await tester.pump(const Duration(milliseconds: 201));
      cursor.syncs.first.complete();
      await tester.pump();
      expect(input.events, isEmpty);
      cursor.syncs.last.complete();
      await tester.pump();
      expect(input.events, [('down', button)]);
      await second.up();
      expect(input.events, [('down', button), ('up', button)]);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(milliseconds: 1));
      expect(input.events, [('down', button), ('up', button)]);
    });
  }
}
