import 'dart:ui' show PointerDeviceKind, SemanticsAction;

import 'package:flutter/gestures.dart' show kSecondaryMouseButton;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const doubleClickInterval = Duration(milliseconds: 75);
  const dragOffset = Offset(150, 0);
  // Keep pointer cases semantics-off to cover parent/child gesture competition.
  // VM taps bypass browser click-to-semantics conversion; verify that in a browser.
  for (final action in ['physical clicks', 'semantic tap', 'keyboard']) {
    testWidgets('More $action opens once without connecting the card',
        (tester) async {
      var opened = 0;
      var connected = 0;
      await tester.pumpWidget(MaterialApp(
        home: Material(
          child: GestureDetector(
            excludeFromSemantics: true,
            onTap: () {},
            onDoubleTap: () => connected++,
            onLongPress: () {},
            child: Builder(builder: (context) {
              return Center(
                  child: build_more(context, onTap: () {
                opened++;
                showMenu<void>(
                  context: context,
                  position: RelativeRect.fill,
                  items: const [PopupMenuItem(child: Text('Connect'))],
                );
              }));
            }),
          ),
        ),
      ));
      final button = find.byIcon(Icons.more_vert);
      final position = tester.getCenter(button);
      if (action == 'semantic tap') {
        final node = tester.getSemantics(button);
        node.owner!.performAction(node.id, SemanticsAction.tap);
      } else if (action == 'keyboard') {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      } else {
        await tester.tapAt(position,
            kind: PointerDeviceKind.mouse, buttons: kSecondaryMouseButton);
        await tester.pumpAndSettle();
        expect(opened, 0, reason: 'A secondary click must not open More');
        await tester.dragFrom(position, dragOffset,
            kind: PointerDeviceKind.mouse);
        await tester.pumpAndSettle();
        expect(opened, 0, reason: 'Dragging out must cancel the More tap');
        await tester.tapAt(position, kind: PointerDeviceKind.mouse);
      }
      expect(opened, 1,
          reason: 'More must not wait for a possible double click');
      await tester.pump();
      if (action == 'physical clicks') {
        await tester.pump(doubleClickInterval);
        await tester.tapAt(position, kind: PointerDeviceKind.mouse);
      }
      await tester.pumpAndSettle();
      expect({'menu': opened, 'connection': connected},
          {'menu': 1, 'connection': 0});
    }, semanticsEnabled: action != 'physical clicks');
  }
}
