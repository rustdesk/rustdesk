import 'package:flutter/material.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final options = {'ll-mouse-btn-pos': '{"x":100.0,"y":800.0}'};
  final reads = <String>[];
  final writes = <String>[];

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) {
    reads.add(k);
    return options[k] ?? '';
  }

  @override
  Future<void> setLocalFlutterOption(
      {required String k, required String v, dynamic hint}) async {
    writes.add(k);
    options[k] = v;
  }
}

class _Cursor extends Fake implements CursorModel {
  @override
  set blockEvents(bool value) {}
  @override
  void addBlockedRect(Rect rect) {}
  @override
  void removeBlockedRect(Rect rect) {}
}

class _Input extends Fake implements InputModel {}

void main() {
  for (final persist in [false, true]) {
    testWidgets(
        persist
            ? 'ordinary mouse buttons retain saved positions'
            : 'half-fold buttons ignore and preserve fullscreen positions',
        (tester) async {
      final binding = _Binding();
      platformFFI.setBindingForTesting(binding);
      final cursor = _Cursor();
      await tester.pumpWidget(MaterialApp(
        home: MediaQuery(
          data: const MediaQueryData(size: Size(669, 250)),
          child: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: 669,
              height: 250,
              child: Stack(children: [
                FloatingLeftRightButton(
                  isLeft: true,
                  inputModel: _Input(),
                  cursorModel: cursor,
                  persistPosition: persist,
                ),
              ]),
            ),
          ),
        ),
      ));
      await tester.pump();
      final target = find.byType(FloatingLeftRightButton);
      final rect = tester.getRect(target);
      if (persist) {
        expect(rect.topLeft, const Offset(100, 800));
        expect(binding.reads, contains('ll-mouse-btn-pos'));
      } else {
        expect((Offset.zero & const Size(669, 250)).contains(rect.topLeft),
            isTrue);
        expect(rect.bottom, lessThanOrEqualTo(250));
        expect(binding.reads, isEmpty);
        await tester.drag(target, const Offset(35, -40));
        await tester.pump();
      }
      await tester.pumpWidget(const SizedBox());
      expect(binding.writes, isEmpty);
      expect(binding.options['ll-mouse-btn-pos'], '{"x":100.0,"y":800.0}');
      expect(tester.takeException(), isNull);
    });
  }
}
