import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Input extends Fake implements InputModel {
  final events = <KeyEvent>[];

  @override
  bool get isViewOnly => false;

  @override
  KeyEventResult handleKeyEvent(KeyEvent event) {
    events.add(event);
    return KeyEventResult.handled;
  }
}

void main() {
  final modifiers = {
    PhysicalKeyboardKey.controlLeft: LogicalKeyboardKey.controlLeft,
    PhysicalKeyboardKey.controlRight: LogicalKeyboardKey.controlRight,
    PhysicalKeyboardKey.altLeft: LogicalKeyboardKey.altLeft,
    PhysicalKeyboardKey.altRight: LogicalKeyboardKey.altRight,
    PhysicalKeyboardKey.shiftLeft: LogicalKeyboardKey.shiftLeft,
    PhysicalKeyboardKey.shiftRight: LogicalKeyboardKey.shiftRight,
    PhysicalKeyboardKey.metaLeft: LogicalKeyboardKey.metaLeft,
    PhysicalKeyboardKey.metaRight: LogicalKeyboardKey.metaRight,
  };

  for (final modifier in modifiers.entries) {
    for (final chatFocused in [false, true]) {
      test('${modifier.key.debugName} release with Chat focus=$chatFocused',
          () {
        final input = _Input();
        final event = KeyUpEvent(
          physicalKey: modifier.key,
          logicalKey: modifier.value,
          timeStamp: Duration.zero,
        );

        expect(
            handleMobileLocalKeyEvent(event,
                remoteFocused: !chatFocused, inputModel: input),
            isFalse);
        expect(input.events, chatFocused ? [event] : isEmpty);
      });
    }
  }

  test('Chat text and modifier presses stay local', () {
    final input = _Input();
    for (final event in [
      const KeyDownEvent(
          physicalKey: PhysicalKeyboardKey.shiftLeft,
          logicalKey: LogicalKeyboardKey.shiftLeft,
          timeStamp: Duration.zero),
      const KeyRepeatEvent(
          physicalKey: PhysicalKeyboardKey.shiftLeft,
          logicalKey: LogicalKeyboardKey.shiftLeft,
          timeStamp: Duration.zero),
      const KeyDownEvent(
          physicalKey: PhysicalKeyboardKey.keyA,
          logicalKey: LogicalKeyboardKey.keyA,
          timeStamp: Duration.zero),
      const KeyUpEvent(
          physicalKey: PhysicalKeyboardKey.keyA,
          logicalKey: LogicalKeyboardKey.keyA,
          timeStamp: Duration.zero),
    ]) {
      expect(
          handleMobileLocalKeyEvent(event,
              remoteFocused: false, inputModel: input),
          isFalse);
    }
    expect(input.events, isEmpty);
  });
}
