import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/display_scale.dart';
import 'package:flutter_hbb/common/widgets/display_settings_dialog.dart';
import 'package:flutter_hbb/models/display_scale_model.dart';
import 'package:flutter_hbb/common.dart' as app;

Widget _displaySettings({
  required FutureOr<void> Function(int, int, int) onApply,
  required VoidCallback onCancel,
  required Future<DisplayScaleState> Function(double, String)? requestScale,
  bool allowArbitrarySize = true,
  bool excludeInputSemantics = false,
}) =>
    MaterialApp(
        home: Scaffold(
            body: SizedBox(
                width: 400,
                child: DisplaySettings(
                    translate: (s) => s,
                    width: 1920,
                    height: 1080,
                    minDimension: 1,
                    maxDimension: 9999,
                    allowArbitrarySize: allowArbitrarySize,
                    excludeInputSemantics: excludeInputSemantics,
                    onApply: onApply,
                    onCancel: onCancel,
                    requestScale: requestScale))));

void main() {
  const state = DisplayScaleState(
      identity: 'display',
      resolution: (1920, 1080),
      percent: 150,
      recommended: 150,
      options: [100, 125, 150, 175, 200],
      token: 'fresh');
  final data = {
    'percent': 150,
    'recommended': 150,
    'options': [100, 125, 150, 175, 200],
    'token': 'fresh',
    'identity': 'display',
    'resolution': [1920, 1080]
  };

  for (final openWithMouse in [false, true]) {
    testWidgets(
        'keyboard scale selection commits only on activation (mouse: $openWithMouse)',
        (tester) async {
      final controller = DisplayScaleModel((_, __) async => state);
      addTearDown(controller.dispose);
      await controller.refresh();
      await tester.pumpWidget(MaterialApp(
          home: Scaffold(
              body: SizedBox(
                  width: 400,
                  child: ListenableBuilder(
                      listenable: controller,
                      builder: (_, __) => DisplayScale(
                          translate: (s) => s, controller: controller))))));
      if (openWithMouse) {
        await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
      } else {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      }
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
      expect(controller.percent, 150);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(controller.percent, 125);
      expect(controller.changed, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(controller.percent, 125);
      expect(
          find.descendant(
              of: find.byKey(const ValueKey('system-scale-menu')),
              matching: find.text('125%')),
          findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }

  test('validates host capabilities', () {
    expect(DisplayScaleState.fromJson(data).options, [100, 125, 150, 175, 200]);
    for (final invalid in [
      {...data, 'percent': 137},
      {...data, 'recommended': 137},
      {
        ...data,
        'options': [100, '125']
      },
      {...data, 'token': ''},
      {...data, 'identity': ''},
      {...data, 'resolution': null},
      {
        ...data,
        'resolution': [0, 1080]
      },
      {
        ...data,
        'options': [0, 150]
      },
      {
        ...data,
        'options': [150, 10000]
      },
    ]) {
      expect(() => DisplayScaleState.fromJson(invalid), throwsFormatException);
    }
  });

  test('response requires matching session and request; duplicate is ignored',
      () async {
    var complete = false;
    final result = DisplayScaleRequests.request('session-a', (id) async {
      final response = jsonEncode({'request_id': id, 'state': data});
      DisplayScaleRequests.handle('session-b', response);
      await Future<void>.delayed(Duration.zero);
      expect(complete, isFalse);
      DisplayScaleRequests.handle('session-a', response);
      DisplayScaleRequests.handle('session-a', response);
    });
    expect((await result).percent, 150);
    complete = true;
  });

  test('malformed response errors retain a translatable message', () async {
    final controller = DisplayScaleModel(
        (_, __) async => DisplayScaleState.fromJson({...data, 'token': ''}));
    addTearDown(controller.dispose);
    expect(await controller.refresh(), isFalse);
    expect(controller.error, 'Invalid display scaling response');
    expect(controller.busy, isFalse);
  });

  test(
      'timeout cleans request and late responses cannot complete another request',
      () async {
    String? id;
    final request = DisplayScaleRequests.request('session', (value) async {
      id = value;
    }, timeout: const Duration(milliseconds: 10));
    await expectLater(request, throwsA(isA<DisplayScaleError>()));
    DisplayScaleRequests.handle(
        'session', jsonEncode({'request_id': id, 'state': data}));
    await expectLater(
        DisplayScaleRequests.request('session', (_) async {
          throw StateError('disconnected');
        }),
        throwsStateError);
  });

  test('decimal capabilities and range validation retain precision', () {
    final fractional = DisplayScaleState.fromJson({
      ...data,
      'percent': 125.5,
      'options': [100, 125.5, 150],
      'custom': [50, 300, 100 / 120]
    });
    expect(fractional.percent, 125.5);
    expect(fractional.options, [100, 125.5, 150]);
    for (final range in [
      [50, 300, 0],
      [300, 50, 1],
      [50, 300, double.nan],
      [50, 300]
    ]) {
      expect(() => DisplayScaleState.fromJson({...data, 'custom': range}),
          throwsFormatException);
    }
  });

  test(
      'native step bounds and invalid custom input never silently change the draft',
      () async {
    const state = DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 125, 150],
        custom: (50, 300, 100 / 120),
        token: 'native');
    final controller = DisplayScaleModel((_, __) async => state);
    addTearDown(controller.dispose);
    await controller.refresh();
    controller.useCustom();
    for (final input in ['', 'NaN', '1%25', '301', '49', '1e2']) {
      controller.edit(input);
      expect(controller.valid, isFalse);
      expect(controller.suggestion, isNull);
      expect(controller.percent, 150);
    }
    controller.edit('125,5');
    expect(controller.valid, isFalse);
    expect(controller.percent, 150);
    expect(controller.suggestion, closeTo(125.833333, 0.000001));
    controller.acceptSuggestion();
    expect(controller.valid, isTrue);
    expect(state.adjacent(50, -1), isNull);
    expect(state.adjacent(300, 1), isNull);
    expect(state.adjacent(125, 1), closeTo(125.833333, 0.000001));
    controller.reset();
    expect(controller.percent, 150);
    expect(controller.customMode, isFalse);
    expect(controller.suggestion, isNull);
  });

  test(
      'failed apply keeps the exact draft and requires a fresh token before retry',
      () async {
    var token = 'old';
    var failApply = true;
    final appliedTokens = <String>[];
    final controller = DisplayScaleModel((percent, expected) async {
      if (percent != 0) {
        appliedTokens.add(expected);
        if (failApply) throw const DisplayScaleError('stale');
      }
      return DisplayScaleState(
          identity: 'display',
          resolution: (1920, 1080),
          percent: percent == 0 ? 150 : percent,
          options: [100, 125, 150],
          custom: (50, 300, 100 / 120),
          token: token);
    });
    addTearDown(controller.dispose);
    await controller.refresh();
    controller.useCustom();
    controller.edit('125.5');
    controller.acceptSuggestion();
    final precise = controller.percent;
    expect(await controller.apply(), isFalse);
    expect(controller.current, isNotNull);
    expect(controller.draftText, '125.83');
    expect(controller.percent, precise);
    expect(await controller.apply(), isFalse);
    expect(appliedTokens, ['old']);
    controller.reset();
    expect(controller.changed, isFalse);
    expect(controller.canApply, isFalse);
    controller.useCustom();
    controller.edit('125.5');
    controller.acceptSuggestion();
    expect(controller.percent, precise);
    expect(controller.canApply, isFalse);
    token = 'new';
    await controller.refresh();
    expect(controller.percent, precise);
    expect(controller.draftText, '125.83');
    expect(controller.customMode, isTrue);
    expect(controller.canApply, isTrue);
    failApply = false;
    expect(await controller.apply(), isTrue);
    expect(appliedTokens, ['old', 'new']);
  });

  test(
      'refresh retains incomplete input and revalidates a removed custom capability',
      () async {
    var custom = true;
    final controller = DisplayScaleModel((_, __) async => DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 150],
        token: 'current',
        custom: custom ? (50, 300, 100 / 120) : null));
    addTearDown(controller.dispose);
    await controller.refresh();
    controller.useCustom();
    controller.edit('125.');
    await controller.refresh();
    expect(controller.draftText, '125.');
    expect(controller.valid, isFalse);
    controller.edit('125.5');
    controller.acceptSuggestion();
    custom = false;
    await controller.refresh();
    expect(controller.draftText, '125.83');
    expect(controller.canApply, isFalse);
    controller.reset();
    expect(controller.draftText, '150');
    expect(controller.canApply, isTrue);
    expect(controller.customMode, isFalse);
  });

  test('refresh adapts preset and custom drafts to new capabilities', () async {
    var native = const DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 125, 150],
        token: 'first');
    final controller = DisplayScaleModel((_, __) async => native);
    addTearDown(controller.dispose);
    await controller.refresh();
    controller.select(125);
    native = const DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 150],
        custom: (50, 300, 1),
        token: 'second');
    await controller.refresh();
    expect(controller.percent, 125);
    expect(controller.customMode, isTrue);
    expect(controller.canApply, isTrue);
    controller.edit('125.5');
    expect(controller.valid, isFalse);
    native = const DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 125.5, 150],
        token: 'third');
    await controller.refresh();
    expect(controller.percent, 125.5);
    expect(controller.customMode, isFalse);
    expect(controller.draftText, '125.5');
    expect(controller.canApply, isTrue);
  });

  for (final withScale in [false, true]) {
    testWidgets(
        'resolution send failure preserves drafts and allows retry (scale: $withScale)',
        (tester) async {
      var closes = 0;
      var scaleRequests = 0;
      var attempts = 0;
      var resolution = (1920, 1080);
      final pending = Completer<void>();
      await tester.pumpWidget(_displaySettings(
          onCancel: () => closes++,
          onApply: (width, height, _) async {
            attempts++;
            if (attempts == 1) await pending.future;
            resolution = (width, height);
          },
          requestScale: !withScale
              ? null
              : (percent, _) async {
                  if (percent != 0) scaleRequests++;
                  return DisplayScaleState(
                      identity: 'display',
                      resolution: resolution,
                      percent: percent == 0 ? 150 : percent,
                      options: [100, 125, 150],
                      token: 'after');
                }));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField).first, '2560');
      await tester.pumpAndSettle();
      if (withScale) {
        await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
        await tester.pumpAndSettle();
        await tester
            .tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
        await tester.pumpAndSettle();
      }
      await tester.tap(find.text('Apply'));
      await tester.pump();
      expect(closes, 0);
      expect(scaleRequests, 0);
      expect(
          tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
          isNull);
      final widthInput =
          tester.widget<EditableText>(find.byType(EditableText).first);
      widthInput.focusNode.requestFocus();
      await tester.pump();
      expect(widthInput.focusNode.hasFocus, isFalse);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(tester.testTextInput.hasAnyClients, isFalse);
      pending.completeError(const DisplayScaleError('send failed'));
      await tester.pumpAndSettle();
      widthInput.focusNode.requestFocus();
      await tester.pump();
      expect(widthInput.focusNode.hasFocus, isTrue);
      expect(closes, 0);
      expect(find.text('send failed'), findsOneWidget);
      expect(scaleRequests, 0);
      expect(
          tester
              .widget<TextField>(find.byType(TextField).first)
              .controller!
              .text,
          '2560');
      await tester.tap(find.text('Apply'));
      await tester.pumpAndSettle();
      expect(attempts, 2);
      expect(closes, 1);
      expect(scaleRequests, withScale ? 1 : 0);
      expect(tester.takeException(), isNull);
    });
  }

  test('mismatched host readback does not report a successful apply', () async {
    final controller = DisplayScaleModel((_, __) async => state);
    addTearDown(controller.dispose);
    expect(await controller.refresh(), isTrue);
    controller.select(125);
    expect(await controller.apply(), isFalse);
    expect(controller.error, contains('did not apply'));
  });

  test('Reset cancels a scale draft matching the refreshed effective value',
      () async {
    var effective = 150.0;
    final controller = DisplayScaleModel((_, __) async => DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: effective,
        options: [100, 125, 150],
        token: 'current'));
    addTearDown(controller.dispose);
    await controller.refresh();
    controller.select(125);
    effective = 125;
    await controller.refresh();
    expect(controller.percent, controller.current!.percent);
    expect(controller.changed, isTrue);
    controller.reset();
    expect(controller.changed, isFalse);
    expect(await controller.apply(), isFalse);
  });

  testWidgets(
      'custom input needs explicit native suggestion and stepping retains precision',
      (tester) async {
    final calls = <double>[];
    const native = DisplayScaleState(
        identity: 'display',
        resolution: (1920, 1080),
        percent: 150,
        options: [100, 125, 150],
        custom: (50, 300, 100 / 120),
        token: 'native');
    await tester.pumpWidget(_displaySettings(
        excludeInputSemantics: true,
        onCancel: () {},
        onApply: (_, __, ___) {},
        requestScale: (value, token) async {
          calls.add(value);
          return value == 0
              ? native
              : DisplayScaleState(
                  identity: 'display',
                  resolution: (1920, 1080),
                  percent: value,
                  options: [value, 150],
                  token: 'after');
        }));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    await tester
        .tap(find.widgetWithText(MenuItemButton, 'Custom').hitTestable());
    await tester.pumpAndSettle();
    final field = find.byKey(const ValueKey('custom-scale-input'));
    expect(
        find.ancestor(
            of: field,
            matching: find.byWidgetPredicate(
                (widget) => widget is ExcludeSemantics && widget.excluding)),
        findsOneWidget);

    await tester.enterText(field, '125.5');
    await tester.pumpAndSettle();
    expect(
        tester
            .widget<TextField>(find.widgetWithText(TextField, 'Width'))
            .controller!
            .text,
        '1920');
    expect(tester.widget<TextField>(field).controller!.text, '125.5');
    expect(find.text('Use nearest supported scale: 125.83%'), findsOneWidget);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    expect(calls, [0]);
    await tester.tap(find.byKey(const ValueKey('accept-scale-suggestion')));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(field).controller!.text, '125.83');
    tester.widget<TextField>(field).controller!.selection =
        const TextSelection.collapsed(offset: 2);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('accept-scale-suggestion')), findsNothing);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNotNull);
    await tester.tap(find.byKey(const ValueKey('scale-increase')));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(field).controller!.text, '126.67');
    await tester.tap(find.byKey(const ValueKey('scale-decrease')));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(field).controller!.text, '125.83');
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(calls.last, closeTo(151 / 120 * 100, 0.000001));
    expect(tester.takeException(), isNull);
  });

  testWidgets('scale-only hosts do not offer unsupported resolution controls',
      (tester) async {
    var applied = false;
    final calls = <double>[];
    await tester.pumpWidget(_displaySettings(
        allowArbitrarySize: false,
        onCancel: () => applied = true,
        onApply: (_, __, ___) =>
            fail('Unchanged resolution must not be submitted'),
        requestScale: (percent, _) async {
          calls.add(percent);
          return percent == 0
              ? state
              : DisplayScaleState(
                  identity: 'display',
                  resolution: (1920, 1080),
                  percent: percent,
                  options: [100, 125, 150],
                  token: 'new');
        }));
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsNothing);
    expect(find.byKey(const ValueKey('resolution-aspect-ratio')), findsNothing);
    expect(find.text('resolution_fit_local_tip'), findsNothing);
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    expect(calls, [0]);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNotNull);
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(applied, isTrue);
    expect(calls, [0, 125]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'combined Apply confirms resolution and retries capped scaling without resending the mode',
      (tester) async {
    final confirmation = Completer<void>();
    final response = Completer<void>();
    final modes = <(int, int, int)>[];
    final calls = <(double, String)>[];
    var resolution = (1920, 1080);
    var rawRequested = 200.0;
    var scaleRequests = 0;
    var closed = false;
    DisplayScaleState readback() => DisplayScaleState(
        identity: 'display',
        resolution: resolution,
        percent: rawRequested
            .clamp(100, resolution == (1920, 1080) ? 150 : 125)
            .toDouble(),
        options: resolution == (1920, 1080) ? [100, 125, 150] : [100, 125],
        token: resolution == (1920, 1080) ? 'fresh' : 'resized');
    await tester.pumpWidget(_displaySettings(
        onCancel: () => closed = true,
        onApply: (w, h, scale) async {
          modes.add((w, h, scale));
          await confirmation.future;
          resolution = (w, h);
        },
        requestScale: (percent, token) async {
          calls.add((percent, token));
          if (percent == 0) return readback();
          if (++scaleRequests == 1) await response.future;
          rawRequested = percent;
          return readback();
        }));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, '1280');
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pump();
    expect(modes, [(1280, 720, 1)]);
    expect(calls, [(0, '')]);
    expect(closed, isFalse);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    confirmation.complete();
    await tester.pump();
    expect(calls, [(0, ''), (0, ''), (125, 'resized')]);
    expect(closed, isFalse);
    response.completeError(const DisplayScaleError('stale'));
    await tester.pumpAndSettle();
    expect(closed, isFalse);
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '1280');
    expect(rawRequested, 200);
    await tester.tap(find.text('Refresh'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(closed, isTrue);
    expect(scaleRequests, 2);
    expect(rawRequested, 125);
    expect(calls.where((call) => call.$1 != 0),
        [(125, 'resized'), (125, 'resized')]);
    expect(modes, [(1280, 720, 1)]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'failed refresh after resolution confirmation retries only scaling',
      (tester) async {
    final modes = <(int, int, int)>[];
    final calls = <double>[];
    var closed = false;
    await tester.pumpWidget(_displaySettings(
        onCancel: () => closed = true,
        onApply: (width, height, scale) => modes.add((width, height, scale)),
        requestScale: (percent, _) async {
          calls.add(percent);
          if (calls.length == 2) {
            throw const DisplayScaleError('refresh failed');
          }
          return DisplayScaleState(
              identity: 'display',
              resolution: modes.isEmpty ? (1920, 1080) : (2560, 1440),
              percent: percent == 0 ? 150 : percent,
              options: [100, 125, 150],
              token: 'current');
        }));
    await tester.pumpAndSettle();
    final width = find.byType(TextField).first;
    final menu = find.byKey(const ValueKey('system-scale-menu'));
    await tester.enterText(width, '2560');
    await tester.tap(menu);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(closed, isFalse);
    expect(calls, [0, 0]);
    expect(find.text('refresh failed'), findsOneWidget);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    await tester.tap(find.text('Reset changes'));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(width).controller!.text, '2560');
    await tester.enterText(width, '1280');
    await tester.pumpAndSettle();
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    await tester.enterText(width, '2560');
    await tester.tap(menu);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    await tester
        .tap(find.widgetWithText(MenuItemButton, 'Custom').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.text('Refresh'));
    await tester.pumpAndSettle();
    expect(find.text('refresh failed'), findsNothing);
    await tester.enterText(width, '1280');
    await tester.pumpAndSettle();
    expect(
        tester.widget<TextField>(find.byType(TextField).last).controller!.text,
        '1440');
    await tester.enterText(width, '2560');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(closed, isTrue);
    expect(modes, [(2560, 1440, 1)]);
    expect(calls, [0, 0, 0, 125]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'changed resolution rejects a scale no longer supported by the host',
      (tester) async {
    var resolution = (1920, 1080);
    var resolutionsApplied = 0;
    var closed = false;
    final calls = <double>[];
    await tester.pumpWidget(_displaySettings(
        onCancel: () => closed = true,
        onApply: (width, height, _) {
          resolutionsApplied++;
          resolution = (width, height);
        },
        requestScale: (percent, _) async {
          calls.add(percent);
          return resolutionsApplied == 0
              ? state
              : DisplayScaleState(
                  identity: 'display',
                  resolution: resolution,
                  percent: 150,
                  options: [100, 150],
                  token: 'reduced');
        }));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, '1280');
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(closed, isFalse);
    expect(resolutionsApplied, 1);
    expect(calls, everyElement(0));
    await tester.tap(find.text('Reset changes'));
    await tester.pumpAndSettle();
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '1280');
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'scaling menu stays above narrow session dialog and keeps other drafts',
      (tester) async {
    tester.view.physicalSize = const Size(1080, 2400);
    tester.view.devicePixelRatio = 3;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final overlay = app.OverlayKeyState();
    final dialogs = app.OverlayDialogManager()..setOverlayState(overlay);
    addTearDown(dialogs.dismissAll);
    await tester.pumpWidget(MaterialApp(
        theme: app.MyTheme.lightTheme,
        home: Overlay(
            key: overlay.key,
            initialEntries: [OverlayEntry(builder: (_) => const Scaffold())])));
    dialogs.show((_, close, context) => app.CustomAlertDialog(
        title: const Text('Resolution'),
        content: SizedBox(
            width: 400,
            child: DisplaySettings(
                width: 1920,
                height: 1080,
                minDimension: 1,
                maxDimension: 9999,
                onApply: (_, __, ___) {},
                translate: (s) => s,
                onCancel: close,
                requestScale: (_, __) async => state))));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, '2560');
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    final item = find.widgetWithText(MenuItemButton, '125%').hitTestable();
    expect(item.hitTestable(), findsOneWidget);
    final menuRect = tester.getRect(
        find.ancestor(of: item, matching: find.byType(Material)).first);
    final menuButton = find.byKey(const ValueKey('system-scale-menu'));
    final buttonRect = tester.getRect(menuButton);
    expect(menuRect.width, closeTo(buttonRect.width, 1));
    await tester.tap(item);
    await tester.pumpAndSettle();
    expect(tester.widget<OutlinedButton>(menuButton).focusNode!.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.widgetWithText(MenuItemButton, '125%').hitTestable(),
        findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(tester.widget<OutlinedButton>(menuButton).focusNode!.hasFocus, isTrue);
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '2560');
    expect(
        find.descendant(
            of: find.byKey(const ValueKey('system-scale-menu')),
            matching: find.text('125%')),
        findsOneWidget);
    await tester.tap(find.text('Reset changes'));
    await tester.pumpAndSettle();
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '1920');
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    expect(tester.takeException(), isNull);
    tester.view.physicalSize = const Size(1680, 1920);
    await tester.pumpAndSettle();
    await tester.tap(menuButton);
    await tester.pumpAndSettle();
    final resizedItem = find.widgetWithText(MenuItemButton, '125%').hitTestable();
    final resizedMenu = tester.getRect(
        find.ancestor(of: resizedItem, matching: find.byType(Material)).first);
    expect(resizedMenu.width, closeTo(tester.getRect(menuButton).width, 1));
    expect(tester.takeException(), isNull);
  });
}
