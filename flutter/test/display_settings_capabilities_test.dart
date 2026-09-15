import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/display_settings_dialog.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/display_scale_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Session implements FFI {
  @override
  final sessionId = UuidValue('00000000-0000-0000-0000-000000000000');

  @override
  ConnType connType = ConnType.defaultConn;

  @override
  late final _DisplayModel ffiModel = _DisplayModel(this);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _DisplayModel extends FfiModel {
  _DisplayModel(FFI ffi) : super(WeakReference(ffi));

  bool controlAllowed = true;
  bool readOnly = false;

  @override
  bool get keyboard => controlAllowed;

  @override
  bool get viewOnly => readOnly;
}

void main() {
  late _Session session;
  late _DisplayModel model;

  setUp(() {
    session = _Session();
    model = session.ffiModel;
    model.pi.currentDisplay = 0;
    model.pi.displays.value = [Display()];
  });

  tearDown(() => model.dispose());

  test('capabilities never bypass control permissions or display selection',
      () {
    model.pi.resolutions.add(Resolution(1920, 1080));
    model.pi.platformAdditions['display_scale'] = true;
    for (final type in ConnType.values) {
      session.connType = type;
      expect(canChangeDisplaySettings(session), type == ConnType.defaultConn);
    }
    session.connType = ConnType.defaultConn;
    model.controlAllowed = false;
    expect(canChangeDisplaySettings(session), isFalse);
    model.controlAllowed = true;
    model.readOnly = true;
    expect(canChangeDisplaySettings(session), isFalse);
    model.readOnly = false;
    model.pi.currentDisplay = -1;
    expect(canChangeDisplaySettings(session), isFalse);
    model.pi.currentDisplay = 0;
    model.pi.displays.clear();
    expect(canChangeDisplaySettings(session), isFalse);
  });

  DisplayScaleState scaleState(double value, String token) => DisplayScaleState(
      percent: value, options: const [100, 125, 150], token: token);

  for (final (platform, scaledWidth, current, fitted) in [
    (kPeerPlatformLinux, 2560, (3840, 2160), (2560, 1440)),
    (kPeerPlatformWindows, 3840, (3840, 2160), (2560, 1440)),
    (kPeerPlatformMacOS, 1920, (1920, 1080), (1280, 720)),
    (kPeerPlatformMacOS, 3840, (3840, 2160), (2560, 1440)),
  ]) {
    testWidgets(
        '$platform current, reset and local fit use native mode units '
        '(scaled width: $scaledWidth)', (tester) async {
      model.pi.platform = platform;
      model.pi.displays.value = [
        model.evtToDisplay({
          'width': 3840,
          'height': 2160,
          'scaled_width': scaledWidth,
        })
      ];
      final target = DisplaySettingsTarget(
          session, (_, __, ___) async => fail('No system scaling request'));
      expect(target.resolution, current);
      final modes = <(int, int, int)>[];
      await tester.pumpWidget(MaterialApp(
          home: Scaffold(
              body: SizedBox(
                  width: 400,
                  child: DisplaySettings(
                      translate: (s) => s,
                      width: target.resolution.$1,
                      height: target.resolution.$2,
                      outputPixelRatio: target.resolutionPixelRatio,
                      usesLogicalSize: target.usesLogicalSize,
                      minDimension: 1,
                      maxDimension: 4096,
                      allowArbitrarySize: false,
                      supportedResolutions: const [
                        (1280, 720),
                        (1920, 1080),
                        (2560, 1440),
                        (3840, 2160)
                      ],
                      localResolution: const (2560, 1440),
                      onApply: (w, h, scale) => modes.add((w, h, scale)),
                      onCancel: () {})))));
      await tester.pumpAndSettle();
      (int, int) inputs() {
        final fields = tester.widgetList<TextField>(find.byType(TextField));
        return (
          int.parse(fields.first.controller!.text),
          int.parse(fields.last.controller!.text)
        );
      }

      expect(inputs(), current);
      await tester.ensureVisible(find.text('resolution_fit_local_tip'));
      await tester.tap(find.text('resolution_fit_local_tip'));
      await tester.pumpAndSettle();
      expect(inputs(), fitted);
      expect(modes, isEmpty);
      await tester.ensureVisible(find.text('Reset changes'));
      await tester.tap(find.text('Reset changes'));
      await tester.pumpAndSettle();
      expect(inputs(), current);
      await tester.ensureVisible(find.text('resolution_fit_local_tip'));
      await tester.tap(find.text('resolution_fit_local_tip'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Apply'));
      await tester.pumpAndSettle();
      expect(modes, [(fitted.$1, fitted.$2, 1)]);
      expect(tester.takeException(), isNull);
    });
  }

  test('an unrelated display refresh is rejected without querying a new target',
      () async {
    model.pi.resolutions.add(Resolution(1920, 1080));
    final target = DisplaySettingsTarget(
        session, (_, __, ___) async => fail('Do not query a replacement'));
    model.pi.displays.value = [Display()];
    await expectLater(
        target.applyResolution(() {}), throwsA(isA<DisplayScaleError>()));
  });

  for (final refreshBeforeReply in [true, false]) {
    test(
        'own scaling accepts a verified capture refresh (before reply: '
        '$refreshBeforeReply)', () async {
      model.pi.platformAdditions['display_scale'] = true;
      final requests = <double>[];
      final updated = Display();
      final target =
          DisplaySettingsTarget(session, (index, percent, token) async {
        expect(index, 0);
        requests.add(percent);
        if (percent == 125 && refreshBeforeReply) {
          model.pi.displays.value = [updated];
        }
        return scaleState(percent == 0 ? 125 : percent, 'after');
      });
      await target.requestScale(125, 'before');
      if (!refreshBeforeReply) model.pi.displays.value = [updated];
      await target.applyResolution(() {});
      await target.requestScale(150, 'after');
      expect(requests, [125, 0, 150]);
    });
  }

  test('a mismatched native token still rejects a replaced display', () async {
    model.pi.platformAdditions['display_scale'] = true;
    final requests = <double>[];
    final target = DisplaySettingsTarget(session, (_, percent, __) async {
      requests.add(percent);
      return scaleState(125, percent == 0 ? 'another-output' : 'after');
    });
    await target.requestScale(125, 'before');
    model.pi.displays.value = [Display()];
    await expectLater(
        target.requestScale(150, 'after'), throwsA(isA<DisplayScaleError>()));
    expect(requests, [125, 0]);
  });

  test('selection and permissions are checked again after native verification',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final reply = Completer<DisplayScaleState>();
    final target = DisplaySettingsTarget(
        session,
        (_, percent, __) async =>
            percent == 0 ? await reply.future : scaleState(125, 'after'));
    await target.requestScale(125, 'before');
    model.pi.displays.value = [Display()];
    final check = target.applyResolution(() {});
    final rejected = expectLater(check, throwsA(isA<DisplayScaleError>()));
    model.controlAllowed = false;
    reply.complete(scaleState(125, 'after'));
    await rejected;
  });

  test('a display changing again during verification is rejected', () async {
    model.pi.platformAdditions['display_scale'] = true;
    final target = DisplaySettingsTarget(session, (_, percent, __) async {
      if (percent == 0) model.pi.displays.value = [Display()];
      return scaleState(125, 'after');
    });
    await target.requestScale(125, 'before');
    model.pi.displays.value = [Display()];
    await expectLater(
        target.applyResolution(() {}), throwsA(isA<DisplayScaleError>()));
  });

  test('an unsuccessful scale readback cannot authorize a display refresh',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final requests = <double>[];
    final target = DisplaySettingsTarget(session, (_, percent, __) async {
      requests.add(percent);
      return scaleState(100, 'unchanged');
    });
    await target.requestScale(125, 'before');
    model.pi.displays.value = [Display()];
    await expectLater(
        target.applyResolution(() {}), throwsA(isA<DisplayScaleError>()));
    expect(requests, [125]);
  });

  test('closing the dialog during verification prevents a resolution request',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final reply = Completer<DisplayScaleState>();
    final target = DisplaySettingsTarget(
        session,
        (_, percent, __) async =>
            percent == 0 ? await reply.future : scaleState(125, 'after'));
    await target.requestScale(125, 'before');
    model.pi.displays.value = [Display()];
    final apply = target.applyResolution(() => fail('The dialog has closed'));
    final rejected = expectLater(apply, throwsA(isA<DisplayScaleError>()));
    target.close();
    reply.complete(scaleState(125, 'after'));
    await rejected;
  });

  test('closing before dispatch prevents even an unchanged target request',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final target = DisplaySettingsTarget(
        session, (_, __, ___) async => fail('The dialog has closed'));
    final request = target.requestScale(125, 'before');
    final rejected = expectLater(request, throwsA(isA<DisplayScaleError>()));
    target.close();
    await rejected;
  });

  testWidgets(
      'combined Apply survives the capture refresh from its scale change',
      (tester) async {
    model.pi.resolutions
        .addAll([Resolution(1920, 1080), Resolution(2560, 1440)]);
    final requests = <double>[];
    var currentScale = 100.0;
    var token = 'before';
    final target = DisplaySettingsTarget(session, (_, percent, __) async {
      requests.add(percent);
      if (percent != 0) {
        currentScale = percent;
        token = 'after';
        model.pi.displays.value = [Display()];
      }
      return scaleState(currentScale, token);
    });
    final modes = <(int, int, int)>[];
    var closed = false;
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: SizedBox(
                width: 400,
                child: DisplaySettings(
                    translate: (s) => s,
                    width: 1920,
                    height: 1080,
                    minDimension: 1,
                    maxDimension: 4096,
                    requestScale: target.requestScale,
                    onApply: (width, height, scale) async {
                      await target.applyResolution(
                          () => modes.add((width, height, scale)));
                    },
                    onCancel: () => closed = true)))));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, '2560');
    await tester.tap(find.byKey(const ValueKey('system-scale-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, '125%').hitTestable());
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(requests, [0, 125, 0]);
    expect(modes, [(2560, 1440, 1)]);
    expect(closed, isTrue);
    expect(tester.takeException(), isNull);
  });
}
