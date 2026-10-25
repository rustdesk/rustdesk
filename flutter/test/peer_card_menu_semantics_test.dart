import 'dart:ui' show PointerDeviceKind, SemanticsAction;

import 'package:flutter/gestures.dart' show kDoubleTapTimeout;
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const doubleClickInterval = Duration(milliseconds: 75);
  for (final action in ['single click', 'double click', 'semantic tap']) {
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
            child: Builder(
                builder: (context) =>
                    Center(child: build_more(context, onTap: () => opened++))),
          ),
        ),
      ));
      final button = find.byIcon(Icons.more_vert);
      if (action == 'semantic tap') {
        final node = tester.getSemantics(button);
        node.owner!.performAction(node.id, SemanticsAction.tap);
      } else {
        await tester.tap(button, kind: PointerDeviceKind.mouse);
        if (action == 'double click') {
          await tester.pump(doubleClickInterval);
          await tester.tap(button, kind: PointerDeviceKind.mouse);
        }
      }
      await tester.pump(kDoubleTapTimeout);
      await tester.pumpAndSettle();
      expect({'menu': opened, 'connection': connected},
          {'menu': 1, 'connection': 0});
    }, semanticsEnabled: action == 'semantic tap');
  }
}
