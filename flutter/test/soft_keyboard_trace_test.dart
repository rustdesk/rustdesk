import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/mobile/soft_keyboard/soft_keyboard.dart';
import 'package:flutter_test/flutter_test.dart';

import 'soft_keyboard_test_support.dart';

/// The JSON objects of a trace, one per line. Whatever a log put in front of
/// them is skipped, and so are lines without any.
List<Map<String, Object?>> readTrace(Iterable<String> lines) => [
      for (final line in lines)
        if (line.contains('{'))
          jsonDecode(line.substring(line.indexOf('{'))) as Map<String, Object?>
    ];

/// Feeds the editing values of a trace to the interpreter named in the header
/// before them and checks that every value produces the recorded actions.
/// Returns how many values there were.
int replay(Iterable<String> lines, {String? reason}) {
  SoftKeyboardInterpreter? interpreter;
  var values = 0;
  for (final entry in readTrace(lines)) {
    if (entry['trace'] == 'soft-keyboard') {
      expect(entry['level'], 'full',
          reason: 'only a trace with text can be replayed');
      interpreter = interpreterByName(
        entry['interpreter'] as String,
        decodeValue(entry['initial'] as Map<String, Object?>),
      );
      continue;
    }
    expect(interpreter, isNotNull, reason: 'a trace starts with a header');
    final actions = interpreter!.update(decodeValue(entry));
    expect(encodeSoftKeyboardActions(actions, withText: true), entry['act'],
        reason: '${reason ?? ''} step ${entry['n']}');
    values++;
  }
  return values;
}

void main() {
  group('trace', () {
    test('is off unless asked for', () {
      final lines = <String>[];
      final trace = SoftKeyboardTrace(sink: lines.add);
      expect(trace.enabled, isFalse);
      trace.start(newIos(), ev(''), platform: 'ios');
      trace.record(ev('a'), const [SoftKeyboardText('a')]);
      expect(lines, isEmpty);
    });

    test('parses its level, and anything unknown is off', () {
      expect(parseSoftKeyboardTraceLevel('meta'), SoftKeyboardTraceLevel.meta);
      expect(parseSoftKeyboardTraceLevel('full'), SoftKeyboardTraceLevel.full);
      expect(parseSoftKeyboardTraceLevel(''), SoftKeyboardTraceLevel.off);
      expect(parseSoftKeyboardTraceLevel('yes'), SoftKeyboardTraceLevel.off);
    });

    test('at meta level never contains what was typed', () {
      final lines = <String>[];
      final trace = SoftKeyboardTrace(
          level: SoftKeyboardTraceLevel.meta, sink: lines.add);
      final ios = newIos();
      trace.start(ios, ev(''), platform: 'ios');
      for (final spec in ['hunter', 'hunter[mima]', 'hunter密码']) {
        final value = ev(spec);
        trace.record(value, ios.update(value), note: ios.lastNote);
      }

      expect(lines, hasLength(4));
      final all = lines.join('\n');
      for (final secret in ['hunter', 'mima', '密码']) {
        expect(all.contains(secret), isFalse);
      }
      // What is there: lengths, ranges and the kind of each action.
      expect(jsonDecode(lines[1]), {
        'n': 1,
        'len': 1030,
        'sel': [1030, 1030],
        'comp': [-1, -1],
        'act': [
          ['text', 6]
        ],
      });
      expect(jsonDecode(lines[2]), {
        'n': 2,
        'len': 1034,
        'sel': [1034, 1034],
        'comp': [1030, 1034],
        'act': [],
      });
    });

    test('at meta level does not tell a typed 1 from the sentinel', () {
      String line(TextEditingValue value) {
        final lines = <String>[];
        SoftKeyboardTrace(level: SoftKeyboardTraceLevel.meta, sink: lines.add)
            .record(value, const [SoftKeyboardText('x')]);
        return lines.single;
      }

      // The same length, typed as digits and typed as letters.
      expect(line(ev('1111')), line(ev('abcd')));
      expect(line(ev('1q2w')), line(ev('abcd')));
    });

    test('at meta level does not contain a correction of the field either', () {
      final lines = <String>[];
      SoftKeyboardTrace(level: SoftKeyboardTraceLevel.meta, sink: lines.add)
          .record(ev(''), [SoftKeyboardSetField(ev('hunter'))]);
      expect(lines.single.contains('hunter'), isFalse);
      expect((jsonDecode(lines.single) as Map)['act'], [
        [
          'set',
          {
            'len': 1030,
            'sel': [1030, 1030],
            'comp': [-1, -1],
          }
        ]
      ]);
    });

    test('at full level can be replayed as it was written', () {
      final lines = <String>[];
      final trace = SoftKeyboardTrace(
          level: SoftKeyboardTraceLevel.full, sink: lines.add);
      final android = newAndroid();
      trace.start(android, ev(''), platform: 'android');
      for (final spec in ['ㅁ', '마', '만', '만 ', '만 “|”', '만 “나|”', '만 ']) {
        final value = ev(spec);
        trace.record(value, android.update(value), note: android.lastNote);
      }

      expect(lines, hasLength(8));
      expect((jsonDecode(lines.first) as Map)['synthetic'], isFalse);
      expect(replay(lines), 7);
    });

    test('can be replayed straight from a log', () {
      // What ends up in a device log: a prefix on every line, and a new
      // header every time the field is refilled.
      final log = <String>[];
      final previous = debugPrint;
      debugPrint = (String? message, {int? wrapWidth}) =>
          log.add('11-12 09:30:01.123  4242  4242 I flutter : $message');
      addTearDown(() => debugPrint = previous);

      final trace = SoftKeyboardTrace(level: SoftKeyboardTraceLevel.full);
      for (final specs in [
        ['[n]', '[ni]', '你'],
        <String>[],
        ['ab', 'a'],
      ]) {
        final ios = newIos();
        trace.start(ios, ev(''), platform: 'ios');
        for (final spec in specs) {
          final value = ev(spec);
          trace.record(value, ios.update(value), note: ios.lastNote);
        }
      }
      log.insert(
          3, '11-12 09:30:01.200  4242  4242 I flutter : something else');

      expect(log, hasLength(9));
      expect(log.first, contains(SoftKeyboardTrace.tag));
      expect(replay(log), 5);
    });

    test('values and actions survive encoding', () {
      for (final value in [
        ev(''),
        ev('你[hao]'),
        ev('a|b'),
        ev('1', lead: 3),
        const TextEditingValue(text: 'no sentinel'),
        TextEditingValue.empty,
      ]) {
        final json = jsonDecode(
                jsonEncode(encodeSoftKeyboardValue(value, withText: true)))
            as Map<String, Object?>;
        expect(decodeValue(json), value);
      }

      final actions = [
        const SoftKeyboardBackspace(3),
        const SoftKeyboardText('he \n'),
        const SoftKeyboardResetField(),
        SoftKeyboardSetField(ev('ab[c]')),
      ];
      final json = jsonDecode(
              jsonEncode(encodeSoftKeyboardActions(actions, withText: true)))
          as List<Object?>;
      expect(decodeActions(json), actions);
    });
  });

  group('recorded sequences', () {
    final files = Directory('test/fixtures/soft_keyboard')
        .listSync()
        .whereType<File>()
        .where((file) => file.path.endsWith('.jsonl'))
        .toList()
      ..sort((a, b) => a.path.compareTo(b.path));

    test('exist', () {
      expect(files, isNotEmpty);
    });

    for (final file in files) {
      final name = file.uri.pathSegments.last;
      test(name, () {
        final lines = file
            .readAsLinesSync()
            .where((line) => line.trim().isNotEmpty)
            .toList();
        final header = readTrace(lines).first;
        // A sequence written by hand shows what the code does with it. Only
        // one captured on a device shows what a keyboard does. Every fixture
        // has to say which of the two it is.
        expect(header['synthetic'], isA<bool>(),
            reason: '$name must say whether it is synthetic');
        expect(replay(lines, reason: name), greaterThan(0));
      });
    }
  });
}
