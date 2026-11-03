import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/android_soft_keyboard.dart';

void main() {
  testWidgets('Hangul composition is visible without space and no filler leaks',
      (tester) async {
    var remote = '';
    await tester.pumpWidget(MaterialApp(home: Material(
      child: AndroidSoftKeyboardInput(
        onText: (text) => remote += text,
        onBackspace: () => remote = remote.characters.skipLast(1).join(),
        onEnter: () => remote += '\n',
      ),
    )));
    await tester.pump();
    expect(tester.widget<EditableText>(find.byType(EditableText)).controller.text,
        isEmpty);
    for (final text in ['a', 'ab', 'abc ', 'abc ㄱ', 'abc 가',
      'abc 가ㄴ', 'abc 가나', 'abc 가나ㄷ', 'abc 가나다']) {
      tester.testTextInput.updateEditingValue(TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
        composing: TextRange(start: text.length - 1, end: text.length),
      ));
      await tester.pump();
      expect(remote, text);
    }
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'abc 가나다', selection: TextSelection.collapsed(offset: 7)));
    await tester.pump();
    expect(remote, 'abc 가나다');
  });

  testWidgets('raw Backspace deletes existing and locally typed remote text',
      (tester) async {
    var remote = '기존';
    await tester.pumpWidget(MaterialApp(home: Material(
      child: AndroidSoftKeyboardInput(
        onText: (text) => remote += text,
        onBackspace: () => remote = remote.characters.skipLast(1).join(),
        onEnter: () => remote += '\n',
      ),
    )));
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
    expect(remote, '기');
    await tester.enterText(find.byType(TextField), 'abc 가나다');
    expect(remote, '기abc 가나다');
    await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
    expect(remote, '기abc 가나');
    await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
    expect(remote, '기abc 가');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'abc 가\n', selection: TextSelection.collapsed(offset: 6)));
    await tester.pump();
    expect(remote, '기abc 가\n');
    expect(remote.codeUnits.last, 10);
    await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
    expect(remote, '기abc 가');
  });

  testWidgets('held Delete and Enter run once per event and real 1 is preserved',
      (tester) async {
    var remote = 'abcdef';
    var remoteKeys = 0;
    await tester.pumpWidget(MaterialApp(home: Material(child: Focus(
      onKeyEvent: (_, __) {
        remoteKeys++;
        return KeyEventResult.handled;
      },
      child: AndroidSoftKeyboardInput(
        onText: (text) => remote += text,
        onBackspace: () => remote = remote.characters.skipLast(1).join(),
        onEnter: () => remote += '\n',
      ),
    ))));
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.backspace);
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.backspace);
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.backspace);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.backspace);
    expect(remote, 'abc');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '\n', selection: TextSelection.collapsed(offset: 1)));
    await tester.pump();
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '\n1', selection: TextSelection.collapsed(offset: 2)));
    await tester.pump();
    expect(remote, 'abc\n1');
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    expect(remoteKeys, 0);
  });
  testWidgets('raw Enter and IME paragraph commits send one real newline',
      (tester) async {
    const sentence = 'abc 가나다 동해물과 백두산이';
    final newline = String.fromCharCode(10);
    for (final imeFirst in [false, true]) {
      var remote = '';
      await tester.pumpWidget(MaterialApp(home: Material(
        key: ValueKey(imeFirst),
        child: AndroidSoftKeyboardInput(
          onText: (text) => remote += text,
          onBackspace: () => remote = remote.characters.skipLast(1).join(),
          onEnter: () => remote += newline,
        ),
      )));
      await tester.pump();
      await tester.enterText(find.byType(TextField), sentence);
      void imeCommit() => tester.testTextInput.updateEditingValue(
          TextEditingValue(text: '$sentence$newline',
              selection: TextSelection.collapsed(
                  offset: sentence.length + 1)));
      if (imeFirst) imeCommit();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      if (!imeFirst) imeCommit();
      await tester.pump();
      // Duplicate platform notifications carry the same complete paragraph.
      imeCommit();
      await tester.pump();
      expect(remote, '$sentence$newline');
      expect(remote.codeUnits.where((unit) => unit == 10).length, 1);
      // A second intentional Enter and typing after it must still work.
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      tester.testTextInput.updateEditingValue(TextEditingValue(
          text: '$sentence$newline$newline',
          selection: TextSelection.collapsed(offset: sentence.length + 2)));
      await tester.pump();
      tester.testTextInput.updateEditingValue(TextEditingValue(
          text: '$sentence$newline${newline}끝',
          selection: TextSelection.collapsed(offset: sentence.length + 3)));
      await tester.pump();
      expect(remote, '$sentence$newline${newline}끝');
      await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
      expect(remote, '$sentence$newline$newline');
    }
  });

  test('rapid Hangul replacements await Delete before Unicode completion', () async {
    var remote = '';
    var previous = '';
    var active = 0;
    var peak = 0;
    final errors = <Object>[];
    final queue = AndroidKeyboardQueue(onError: errors.add);
    var completed = Future<void>.value();
    Future<void> send(void Function() action, int delay) async {
      active++;
      if (active > peak) peak = active;
      await Future<void>.delayed(Duration(milliseconds: delay));
      action();
      active--;
    }
    for (final value in ['ㄱ', '가', '간', '가나', '가나다', '가나다 동해물과 백두산이']) {
      final edit = computeAndroidSoftKeyboardEdit(previous, value);
      previous = value;
      for (var i = 0; i < edit.deleteCount; i++) {
        completed = queue.add(() => send(
            () => remote = remote.characters.skipLast(1).join(), 8));
      }
      if (edit.insertedText.isNotEmpty) {
        completed = queue.add(() => send(() => remote += edit.insertedText, 1));
      }
    }
    await completed;
    expect(remote, '가나다 동해물과 백두산이');
    expect(peak, 1);
    expect(errors, isEmpty);
  });

  test('input failure stops later commands and closed queues send nothing', () async {
    final errors = <Object>[];
    final queue = AndroidKeyboardQueue(onError: errors.add);
    var sent = 0;
    queue.add(() async => throw StateError('send failed'));
    await queue.add(() async { sent++; });
    expect(errors, hasLength(1));
    expect(sent, 0);
    final closed = AndroidKeyboardQueue(onError: errors.add);
    closed.add(() async { sent++; });
    closed.close();
    await closed.add(() async { sent++; });
    expect(sent, 0);
  });

}
