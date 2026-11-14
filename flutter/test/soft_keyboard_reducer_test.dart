// All sequences here are written by hand. They show what the interpreter does
// with a given sequence, not that a keyboard produces that sequence.

import 'dart:math';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/soft_keyboard/soft_keyboard.dart';
import 'package:flutter_test/flutter_test.dart';

import 'soft_keyboard_test_support.dart';

void main() {
  group('iOS: composing text is held', () {
    test('a composition sends only what is committed', () {
      expect(
          typed(newIos(), ['[n]', '[ni]', '[nih]', '[niha]', '[nihao]', '你好']),
          ['你好']);
    });

    test('a partial commit sends exactly the committed part', () {
      expect(typed(newIos(), ['[n]', '[nihao]', '你[hao]', '你好']), ['你', '好']);
    });

    test('Backspace inside a composition sends nothing', () {
      expect(typed(newIos(), ['[n]', '[ni]', '[nih]', '[ni]', '你']), ['你']);
    });

    test('a commit that does not change the text is sent', () {
      expect(typed(newIos(), ['[你]', '你']), ['你']);
      expect(typed(newIos(), ['[あ]', '[あり]', '[ありがとう]', 'ありがとう']), ['ありがとう']);
    });

    test('a cancelled composition sends nothing', () {
      expect(typed(newIos(), ['[n]', '[ni]', '']), isEmpty);
    });

    test('a composition after committed text leaves that text alone', () {
      expect(typed(newIos(), ['ab', 'ab[ni]', 'ab你']), ['ab', '你']);
    });

    test('committing the letters as they are sends them once', () {
      // Return on the Pinyin keyboard commits the letters typed so far.
      expect(typed(newIos(), ['[ni]', '[nihao]', 'nihao']), ['nihao']);
    });

    test('the same text committed twice is sent twice', () {
      expect(typed(newIos(), ['[ni]', '你', '你[ni]', '你你']), ['你', '你']);
    });

    test('a change of selection alone sends nothing', () {
      expect(typed(newIos(), ['ab', 'a|b', '|ab', 'ab']), ['ab']);
    });

    test('composing text that covers sent text is followed, not retracted', () {
      final ios = newIos();
      // The keyboard marks the committed character as composing again.
      expect(typed(ios, ['你', '[你]']), ['你']);
      expect(ios.lastNote, 'composing covers sent text');
      // And then replaces it.
      expect(typed(ios, ['尼']), ['<BS>', '尼']);
    });
  });

  group('iOS: without a composing range nothing changes', () {
    test('plain typing', () {
      expect(typed(newIos(), ['a', 'ab', 'ab ', 'ab c']), ['a', 'b', ' ', 'c']);
    });

    test('a replaced tail', () {
      expect(typed(newIos(), ['t', 'te', 'teh', 'the ']),
          ['t', 'e', 'h', '<BS*2>', 'he ']);
    });

    test('Backspace, also into the sentinel, and the digit 1 afterwards', () {
      final ios = newIos();
      expect(typed(ios, ['ab', 'a', '']), ['ab', '<BS>', '<BS>']);
      expect(
          sent(ios, [ev('', lead: 1023), ev('', lead: 1024)]), ['<BS>', '1']);
    });

    test('Return and Space', () {
      expect(typed(newIos(), ['a', 'a\n', 'a\n ']), ['a', '\n', ' ']);
    });

    // The claim behind enabling this on iOS by default: fed the same editing
    // values, none of which has a composing range, the new interpreter and
    // the old handler send the same thing at every step.
    test('matches the old handler on random editing sequences', () {
      final random = Random(16632);
      for (var run = 0; run < 400; run++) {
        final oldHandler = legacyIos();
        final newHandler = newIos();
        var lead = 1024;
        var rest = '';
        for (var step = 0; step < 60; step++) {
          final roll = random.nextInt(10);
          var caret = rest.length;
          if (roll < 5) {
            rest += _randomText(random, 1 + random.nextInt(3));
          } else if (roll < 7 && rest.isNotEmpty) {
            rest = rest.substring(0, rest.length - 1);
          } else if (roll == 7 && rest.isNotEmpty) {
            // Something like an autocorrection: the tail is replaced.
            final cut = 1 + random.nextInt(min(4, rest.length));
            rest = rest.substring(0, rest.length - cut) +
                _randomText(random, random.nextInt(4));
          } else if (roll == 8 && rest.isEmpty && lead > 0) {
            lead--;
          } else {
            // Only the caret moves.
            caret = random.nextInt(rest.length + 1);
          }
          final value = plain(rest, lead: lead, caret: caret);
          expect(tokens(newHandler.update(value)),
              tokens(oldHandler.update(value)),
              reason: 'run $run step $step: "$rest" with $lead sentinels');
        }
      }
    });

    // And with compositions mixed in: what has been sent is always exactly
    // the committed text. Nothing provisional, nothing missing.
    test('the remote text is always the committed text', () {
      final random = Random(8471);
      for (var run = 0; run < 400; run++) {
        final ios = newIos();
        final remote = RemoteText(backspaceDeletesCodePoint: false);
        var committed = '';
        var composing = '';
        for (var step = 0; step < 60; step++) {
          final roll = random.nextInt(10);
          if (composing.isEmpty) {
            if (roll < 4) {
              composing = _randomLetters(random, 1);
            } else if (roll < 7) {
              committed += _randomBmpText(random, 1 + random.nextInt(2));
            } else if (committed.isNotEmpty) {
              committed = committed.substring(0, committed.length - 1);
            }
          } else {
            if (roll < 4) {
              composing += _randomLetters(random, 1);
            } else if (roll < 5) {
              composing = composing.substring(0, composing.length - 1);
            } else if (roll < 7) {
              // The whole composition is converted and committed.
              committed += _randomCjk(random, 1 + random.nextInt(2));
              composing = '';
            } else if (roll < 9 && composing.length > 1) {
              // The first part is committed, the rest keeps composing.
              committed += _randomCjk(random, 1);
              composing = composing.substring(1);
            } else {
              // Committed as typed. The text does not change.
              committed += composing;
              composing = '';
            }
          }
          final value =
              ev(composing.isEmpty ? committed : '$committed[$composing]');
          remote.apply(ios.update(value));
          expect(remote.text, committed, reason: 'run $run step $step');
          expect(remote.overrun, 0, reason: 'run $run step $step');
        }
      }
    });
  });

  group('Android: the field is mirrored', () {
    test('plain typing and deleting', () {
      expect(typed(newAndroid(), ['a', 'ab', 'ab ', 'ab c', 'ab ']),
          ['a', 'b', ' ', 'c', '<BS>']);
    });

    test('text committed at once is sent as a string', () {
      expect(typed(newAndroid(), ['你好']), ['你好']);
    });

    test('a syllable edited in place is replaced (#7727)', () {
      expect(typed(newAndroid(), ['ㅁ', '마', '만', '마나']),
          ['ㅁ', '<BS>', '마', '<BS>', '만', '<BS>', '마나']);
    });

    test('a syllable is replaced whether or not it is marked as composing', () {
      expect(typed(newAndroid(), ['[ㅁ]', '[마]', '[만]', '만[ㄴ]']),
          ['ㅁ', '<BS>', '마', '<BS>', '만', 'ㄴ']);
    });

    test('composing letters are replaced by the committed text', () {
      expect(
          typed(newAndroid(),
              ['[n]', '[ni]', '[nih]', '[niha]', '[nihao]', '你好']),
          ['n', 'i', 'h', 'a', 'o', '<BS*5>', '你好']);
    });

    test('a word replaced by a suggestion', () {
      expect(typed(newAndroid(), ['[teh]', 'the ']), ['teh', '<BS*2>', 'he ']);
    });

    test('text inserted inside a pair of quotes (#16535)', () {
      // The keyboard leaves the caret between the quotes, so what comes next
      // is not appended.
      expect(typed(newAndroid(), ['“|”', '“你好|”', '“你好吗|”']),
          ['“”', '<BS>', '你好”', '<BS>', '吗”']);
    });

    test('a pair of brackets needs no field reset', () {
      expect(
          sent(newAndroid(),
              [plain('a'), plain('a()', caret: 2), plain('a(b)', caret: 3)]),
          ['a', '()', '<BS>', 'b)']);
    });

    test('text deleted before the caret', () {
      expect(typed(newAndroid(), ['(ab|)', '(a|)']), ['(ab)', '<BS*2>', ')']);
    });

    test('a deleted emoji is one Backspace', () {
      expect(typed(newAndroid(), ['😀', '']), ['😀', '<BS>']);
      expect(typed(newAndroid(), ['a👨‍👩‍👧', 'a']), ['a👨‍👩‍👧', '<BS>']);
    });

    test('several characters deleted at once are that many Backspaces', () {
      expect(typed(newAndroid(), ['one two', 'one ']), ['one two', '<BS*3>']);
    });

    test('an emoji replaced by another one', () {
      expect(typed(newAndroid(), ['😀', '😁']), ['😀', '<BS>', '😁']);
    });

    test('a change of selection or composing range alone sends nothing', () {
      expect(typed(newAndroid(), ['ab', 'a|b', '[ab]', 'ab']), ['ab']);
    });

    group('the sentinel', () {
      test('is deleted one Backspace at a time', () {
        final android = newAndroid();
        expect(sent(android, [ev('', lead: 1023), ev('', lead: 1022)]),
            ['<BS>', '<BS>']);
      });

      test('is one Backspace however much of it is deleted at once', () {
        expect(sent(newAndroid(), [ev('', lead: 0)]), ['<BS>']);
        expect(sent(newAndroid(), [ev('', lead: 1000)]), ['<BS>']);
      });

      test('is never typed back', () {
        final android = newAndroid();
        expect(sent(android, [ev('', lead: 1023), ev('가', lead: 1023)]),
            ['<BS>', '가']);
        expect(sent(android, [ev('각', lead: 1023)]), ['<BS>', '각']);
      });

      test('does not swallow the digit 1', () {
        final android = newAndroid();
        expect(typed(android, ['1', '11', '1']), ['1', '1', '<BS>']);
        // Also after the sentinel got shorter.
        expect(sent(android, [ev(''), ev('', lead: 1023), ev('', lead: 1024)]),
            ['<BS>', '<BS>', '1']);
      });

      test('deleted together with typed text counts the typed text', () {
        expect(sent(newAndroid(), [ev('ab'), ev('', lead: 1021)]),
            ['ab', '<BS*3>']);
      });

      test('replaced as a whole sends the new text only', () {
        final android = newAndroid();
        expect(sent(android, [const TextEditingValue(text: 'pasted text')]),
            ['pasted text']);
        expect(android.lastNote, 'at caret: edit inside the sentinel');
      });

      test('typed into sends the text and is not replayed afterwards', () {
        final android = newAndroid();
        // The caret was moved into the sentinel.
        TextEditingValue inSentinel(String typed) => TextEditingValue(
            text: '${'1' * 1000}$typed${'1' * 24}',
            selection: TextSelection.collapsed(offset: 1000 + typed.length));
        expect(sent(android, [inSentinel('a')]), ['a']);
        // No Backspaces through the 24 characters after the caret.
        expect(sent(android, [inSentinel('ab')]), ['b']);
        expect(sent(android, [inSentinel('a')]), ['<BS>']);
      });
    });

    group('what cannot be replayed', () {
      test('a replacement longer than the limit is dropped', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        expect(typed(android, ['abcde', 'ABCDE']), ['abcde']);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
      });

      test('a replacement inside a character cluster is dropped', () {
        final android = newAndroid();
        // The skin tone changes. Applications disagree on whether Backspace
        // removes the modifier or the whole emoji.
        expect(typed(android, ['👍🏽', '👍🏿']), ['👍🏽']);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
      });

      test('only freezes the text up to that point', () {
        final android = newAndroid();
        expect(typed(android, ['👍🏽', '👍🏿']), ['👍🏽']);
        // Typing goes on, and what is typed from here on is replayed again.
        expect(typed(android, ['👍🏿ㅁ', '👍🏿마']), ['ㅁ', '<BS>', '마']);
        // Backspace still is Backspace.
        expect(typed(android, ['👍🏿', '']), ['<BS>', '<BS>']);
      });

      test('is not edited later either', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        // The remote side keeps "abcde". From here on a position in the
        // local text says nothing about a position in the remote text.
        expect(typed(android, ['abcde', 'XY']), ['abcde']);
        expect(typed(android, ['XZ']), isEmpty);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
        // What is typed after it is mirrored again.
        expect(typed(android, ['XZw', 'XZv']), ['w', '<BS>', 'v']);
      });

      test('a line break is never deleted and typed again', () {
        final android = newAndroid();
        expect(typed(android, ['a\n', 'b\n']), ['a', '\n']);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
        // Typing in front of it goes to the caret instead.
        expect(typed(android, ['bX|\n']), ['X']);
        expect(android.lastNote,
            'at caret: text after the edit cannot be replayed');
      });

      test('Return typed with other text is still sent on its own', () {
        // On its own it goes out as the key. Inside a string it would only
        // be a character.
        expect(typed(newAndroid(), ['teh', 'the\n']),
            ['teh', '<BS*2>', 'he', '\n']);
        expect(
            typed(newAndroid(), ['(|)', '(\n|)']), ['()', '<BS>', '\n', ')']);
        expect(typed(newAndroid(), ['a', 'ab\n']), ['a', 'b', '\n']);
      });

      test('a line break inside committed text stays text', () {
        expect(typed(newAndroid(), ['one\ntwo']), ['one\ntwo']);
      });

      test('Backspace after Return is still one Backspace', () {
        expect(typed(newAndroid(), ['a', 'a\n', 'a']), ['a', '\n', '<BS>']);
      });

      test('a replacement in front of too much text is dropped', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        expect(typed(android, ['teh abcdef', 'the| abcdef']), ['teh abcdef']);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
      });

      test('Backspace in front of too much text is still Backspace', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        expect(typed(android, ['xabcdef', '|abcdef']), ['xabcdef', '<BS>']);
        expect(android.lastNote,
            'at caret: text after the edit cannot be replayed');
      });

      test('an insertion in front of too much text is sent on its own', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        expect(typed(android, ['abcdef', 'X|abcdef']), ['abcdef', 'X']);
        expect(android.lastNote,
            'at caret: text after the edit cannot be replayed');
      });

      test('a deletion is never more Backspaces than the limit', () {
        final android = SoftKeyboardReducer(
          policy: ComposingPolicy.forward,
          diff: MirrorSoftKeyboardDiff(maxReplay: 4),
          initial: ev(''),
        );
        expect(typed(android, ['abcdefgh', 'ab']), ['abcdefgh', '<BS*4>']);
        expect(android.lastNote, 'clamped: more Backspaces than the limit');
        // The remote side still has "abgh". What is left of it is not edited.
        expect(typed(android, ['ac']), isEmpty);
        expect(android.lastNote, 'dropped: replacement cannot be replayed');
        expect(typed(android, ['acd', 'ace']), ['d', '<BS>', 'e']);
      });
    });

    // On sequences the old handler gets right, appending and deleting one
    // character at the end, the new interpreter sends the same thing.
    test('matches the old handler on appending and deleting', () {
      final random = Random(9790);
      for (var run = 0; run < 400; run++) {
        final oldHandler = legacyAndroid();
        final newHandler = newAndroid();
        var lead = 1024;
        var rest = '';
        for (var step = 0; step < 60; step++) {
          final roll = random.nextInt(10);
          if (roll < 6) {
            // No brackets: the old handler resets the field after a pair.
            // And Return only on its own: typed together with other text,
            // the old handler sends it as a character in a string.
            var chunk = _randomBmpText(random, 1 + random.nextInt(3))
                .replaceAll(RegExp(r'[()]'), 'x');
            if (chunk.length > 1) chunk = chunk.replaceAll('\n', ' ');
            rest += chunk;
          } else if (rest.isNotEmpty) {
            rest = rest.substring(0, rest.length - 1);
          } else if (lead > 0) {
            lead--;
          }
          final value = plain(rest, lead: lead);
          expect(tokens(newHandler.update(value)),
              tokens(oldHandler.update(value)),
              reason: 'run $run step $step: "$rest" with $lead sentinels');
        }
      }
    });

    // Whatever is done to the typed text, at the end or in the middle, the
    // remote text stays equal to it. Within one line, that is: Return is a
    // key press and is never replayed.
    test('the remote text always equals the typed text', () {
      final random = Random(7727);
      String oneLine(Random random, int length) =>
          _randomBmpText(random, length).replaceAll('\n', ' ');
      for (var run = 0; run < 400; run++) {
        final android = newAndroid();
        final remote = RemoteText(backspaceDeletesCodePoint: true);
        var rest = '';
        for (var step = 0; step < 60; step++) {
          final roll = random.nextInt(10);
          // Keep within what may be replayed.
          final from = max(0, rest.length - 20);
          final at = from + random.nextInt(rest.length - from + 1);
          if (roll < 4 || rest.isEmpty) {
            rest += oneLine(random, 1 + random.nextInt(3));
          } else if (roll < 6) {
            rest = rest.substring(0, rest.length - 1);
          } else if (roll < 8) {
            // Replace from somewhere to the end.
            rest = rest.substring(0, at) + oneLine(random, random.nextInt(4));
          } else if (roll < 9) {
            // Insert somewhere.
            rest = rest.substring(0, at) +
                oneLine(random, 1 + random.nextInt(2)) +
                rest.substring(at);
          } else if (at < rest.length) {
            // Delete somewhere.
            rest = rest.substring(0, at) + rest.substring(at + 1);
          }
          remote.apply(android.update(plain(rest)));
          expect(remote.text, rest, reason: 'run $run step $step');
          expect(remote.overrun, 0, reason: 'run $run step $step');
          expect(android.lastNote, isNull, reason: 'run $run step $step');
        }
      }
    });
  });

  group('Android keyboards that only compose real compositions', () {
    // Not used yet. Holding composing text on Android needs a way to tell
    // such a keyboard from one that also composes plain words.
    SoftKeyboardInterpreter holdAndMirror() =>
        interpreterByName('hold+mirror', ev(''));

    test('a composition sends only what is committed', () {
      expect(typed(holdAndMirror(), ['[n]', '[nihao]', '你[hao]', '你好']),
          ['你', '好']);
    });

    test('a syllable is sent when it is complete', () {
      expect(
          typed(holdAndMirror(), ['[ㅁ]', '[마]', '[만]', '만[ㄴ]', '만[나]', '만나']),
          ['만', '나']);
    });
  });

  group('choosing the interpreter', () {
    test('by platform and option', () {
      String name({required bool isIOS, required bool useReducer}) =>
          createSoftKeyboardInterpreter(
                  isIOS: isIOS, useReducer: useReducer, initial: ev(''))
              .name;
      expect(name(isIOS: true, useReducer: true), 'hold+tail');
      expect(name(isIOS: true, useReducer: false), 'legacy-ios');
      expect(name(isIOS: false, useReducer: true), 'forward+mirror');
      expect(name(isIOS: false, useReducer: false), 'legacy-android');
    });
  });
}

const _letters = 'abcdefghijklmnopqrstuvwxyz';
const _cjk = '你好中文日本語한국어가나다';
const _bmp = '${_letters}ABC 1.,\n()$_cjk';
const _emoji = ['😀', '🎉'];

String _randomLetters(Random random, int length) => String.fromCharCodes([
      for (var i = 0; i < length; i++)
        _letters.codeUnitAt(random.nextInt(_letters.length))
    ]);

String _randomCjk(Random random, int length) => String.fromCharCodes([
      for (var i = 0; i < length; i++)
        _cjk.codeUnitAt(random.nextInt(_cjk.length))
    ]);

/// Characters of one UTF-16 code unit each.
String _randomBmpText(Random random, int length) => String.fromCharCodes([
      for (var i = 0; i < length; i++)
        _bmp.codeUnitAt(random.nextInt(_bmp.length))
    ]);

/// Like [_randomBmpText], with the occasional emoji.
String _randomText(Random random, int length) => [
      for (var i = 0; i < length; i++)
        random.nextInt(12) == 0
            ? _emoji[random.nextInt(_emoji.length)]
            : _randomBmpText(random, 1)
    ].join();
