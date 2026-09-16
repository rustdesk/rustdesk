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

  DisplayScaleState scaleState(double value, String token,
          {(int, int) resolution = (1920, 1080),
          String identity = 'display'}) =>
      DisplayScaleState(
          percent: value,
          options: const [100, 125, 150],
          token: token,
          identity: identity,
          resolution: resolution);

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
      final target = DisplaySettingsTarget(session,
          (_, __, ___, ____) async => fail('No system scaling request'));
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
                      onApply: (w, h, scale) {
                        modes.add((w, h, scale));
                        return null;
                      },
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
    final target = DisplaySettingsTarget(session,
        (_, __, ___, ____) async => fail('Do not query a replacement'));
    model.pi.displays.value = [Display()];
    await expectLater(
        target.applyResolution(() {}), throwsA(isA<DisplayScaleError>()));
  });

  test(
      'a capture replacement during the initial read cannot establish identity',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final entered = Completer<void>();
    final reply = Completer<DisplayScaleState>();
    var reads = 0;
    final target = DisplaySettingsTarget(session, (_, __, ___, ____) async {
      reads++;
      entered.complete();
      return await reply.future;
    });
    final initial = target.requestScale(0, '');
    final rejected = expectLater(initial, throwsA(isA<DisplayScaleError>()));
    await entered.future;
    model.pi.displays.value = [Display()];
    reply.complete(scaleState(100, 'replacement', identity: 'replacement'));
    await rejected;
    await expectLater(
        target.requestScale(0, ''), throwsA(isA<DisplayScaleError>()));
    await expectLater(target.requestScale(125, 'replacement'),
        throwsA(isA<DisplayScaleError>()));
    expect(reads, 1);
  });

  for (final refreshBeforeReply in [true, false]) {
    test(
        'own scaling accepts a verified capture refresh (before reply: '
        '$refreshBeforeReply)', () async {
      model.pi.platformAdditions['display_scale'] = true;
      final requests = <double>[];
      final updated = Display();
      final target = DisplaySettingsTarget(session,
          (index, percent, token, identity) async {
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

  for (final changed in ['token', 'identity']) {
    test('a mismatched native $changed still rejects writes', () async {
      model.pi.platformAdditions['display_scale'] = true;
      final requests = <double>[];
      final target =
          DisplaySettingsTarget(session, (_, percent, __, ___) async {
        requests.add(percent);
        if (percent != 0) return scaleState(125, 'after');
        return scaleState(125, changed == 'token' ? 'another-output' : 'after',
            identity: changed == 'identity' ? 'replacement' : 'display');
      });
      await target.requestScale(125, 'before');
      model.pi.displays.value = [Display()];
      await expectLater(
          target.requestScale(150, 'after'), throwsA(isA<DisplayScaleError>()));
      expect(requests, [125, 0]);
    });
  }

  test('Refresh recovers a lost scaling reply after changing resolution',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    var resolution = (1920, 1080);
    var percent = 100.0;
    var token = 'before';
    var scaleWrites = 0;
    var resolutionWrites = 0;
    final target = DisplaySettingsTarget(session,
        (_, requested, expected, identity) async {
      if (requested != 0) {
        expect(expected, token);
        scaleWrites++;
        percent = requested;
        token = 'scale-$scaleWrites';
        model.pi.displays.value = [Display()];
        if (scaleWrites == 1) {
          throw const DisplayScaleError(
              'Display settings timed out. Refresh and try again.');
        }
      }
      return scaleState(percent, token, resolution: resolution);
    });
    final controller = DisplayScaleModel(target.requestScale);
    addTearDown(controller.dispose);
    expect(await controller.refresh(), isTrue);
    await target.applyResolution(() {
      resolutionWrites++;
      resolution = (2560, 1440);
      token = 'new-mode';
      model.pi.displays.value = [Display()];
    }, resolution: (2560, 1440));
    expect(await controller.refresh(), isTrue);
    controller.select(125);
    expect(await controller.apply(), isFalse);
    expect(controller.needsRefresh, isTrue);
    await expectLater(target.requestScale(150, controller.current!.token),
        throwsA(isA<DisplayScaleError>()));
    expect(scaleWrites, 1);
    expect(await controller.refresh(), isTrue);
    expect(controller.current!.percent, 125);
    expect(controller.current!.resolution, resolution);
    expect(controller.needsRefresh, isFalse);
    expect(controller.canApply, isTrue);
    expect(scaleWrites, 1);
    controller.select(150);
    expect(await controller.apply(), isTrue);
    expect(scaleWrites, 2);
    expect(resolutionWrites, 1);
  });

  for (final changed in ['identity', 'resolution', 'permission', 'capture']) {
    test('Refresh rejects $changed changes during native verification',
        () async {
      model.pi.platformAdditions['display_scale'] = true;
      final reply = Completer<DisplayScaleState>();
      var reads = 0;
      final target =
          DisplaySettingsTarget(session, (_, percent, __, ___) async {
        expect(percent, 0);
        return ++reads == 1 ? scaleState(100, 'before') : await reply.future;
      });
      await target.requestScale(0, '');
      model.pi.displays.value = [Display()];
      final refresh = target.requestScale(0, '');
      final rejected = expectLater(refresh, throwsA(isA<DisplayScaleError>()));
      if (changed == 'permission') model.controlAllowed = false;
      if (changed == 'capture') model.pi.displays.value = [Display()];
      reply.complete(scaleState(125, 'after',
          identity: changed == 'identity' ? 'replacement' : 'display',
          resolution: changed == 'resolution' ? (2560, 1440) : (1920, 1080)));
      await rejected;
      expect(reads, 2);
    });
  }

  test('selection and permissions are checked again after native verification',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    final reply = Completer<DisplayScaleState>();
    final target = DisplaySettingsTarget(
        session,
        (_, percent, __, ___) async =>
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
    final target = DisplaySettingsTarget(session, (_, percent, __, ___) async {
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
    final target = DisplaySettingsTarget(session, (_, percent, __, ___) async {
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
        (_, percent, __, ___) async =>
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
        session, (_, __, ___, ____) async => fail('The dialog has closed'));
    final request = target.requestScale(125, 'before');
    final rejected = expectLater(request, throwsA(isA<DisplayScaleError>()));
    target.close();
    await rejected;
  });

  test(
      'an unconfirmed mode change blocks writes until the requested mode arrives',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    var resolution = (1920, 1080);
    var sends = 0;
    final target = DisplaySettingsTarget(
        session,
        (_, percent, __, ___) async => scaleState(
            percent == 0 ? 100 : percent, 'current',
            resolution: resolution));
    await target.requestScale(0, '');
    final operation =
        target.applyResolution(() => sends++, resolution: (2560, 1440));
    await Future<void>.delayed(Duration.zero);
    await expectLater(
        target.requestScale(0, ''), throwsA(isA<DisplayScaleError>()));
    await expectLater(
        target.requestScale(125, 'current'), throwsA(isA<DisplayScaleError>()));
    await expectLater(
        target.applyResolution(() => sends++, resolution: (2560, 1440)),
        throwsA(isA<DisplayScaleError>()));
    expect(sends, 1);
    resolution = (2560, 1440);
    await operation;
    expect((await target.requestScale(0, '')).resolution, resolution);
    expect((await target.requestScale(125, 'current')).percent, 125);
  });

  test('mode confirmation propagates read errors without retrying', () async {
    model.pi.platformAdditions['display_scale'] = true;
    for (final error in [
      const DisplayScaleError('unsupported', code: 'unsupported'),
      const DisplayScaleError('No permission to change display settings.'),
      const DisplayScaleError(
          'Display settings changed. Reopen the resolution menu and try again.'),
      const FormatException('invalid response'),
    ]) {
      var queries = 0;
      var sends = 0;
      final target = DisplaySettingsTarget(session, (_, __, ___, ____) async {
        queries++;
        if (queries == 2) throw error;
        return scaleState(100, 'current',
            resolution: queries == 1 ? (1920, 1080) : (2560, 1440));
      });
      await target.requestScale(0, '');
      await expectLater(
          target.applyResolution(() => sends++, resolution: (2560, 1440)),
          throwsA(same(error)));
      expect(queries, 2);
      await expectLater(target.requestScale(125, 'current'),
          throwsA(isA<DisplayScaleError>()));
      expect(queries, 2);
      expect((await target.requestScale(0, '')).resolution, (2560, 1440));
      expect(sends, 1);
      target.close();
    }
  });

  test(
      'mode confirmation retries only snapshot changes with the original identity',
      () async {
    model.pi.platformAdditions['display_scale'] = true;
    for (final failure in [
      null,
      const DisplayScaleError('unsupported', code: 'unsupported'),
    ]) {
      final identities = <String>[];
      var sends = 0;
      final target =
          DisplaySettingsTarget(session, (_, percent, __, identity) async {
        expect(percent, 0);
        identities.add(identity);
        if (identities.length == 2) {
          throw const DisplayScaleError('changing', code: 'snapshot_changed');
        }
        if (identities.length == 3 && failure != null) throw failure;
        return scaleState(100, 'current',
            resolution: identities.length == 1 ? (1920, 1080) : (2560, 1440));
      });
      await target.requestScale(0, '');
      final operation =
          target.applyResolution(() => sends++, resolution: (2560, 1440));
      if (failure == null) {
        expect((await operation)!.resolution, (2560, 1440));
      } else {
        await expectLater(operation, throwsA(same(failure)));
      }
      expect(identities, ['', 'display', 'display']);
      expect(sends, 1);
      target.close();
    }
  });

  test('mode confirmation rejects a replaced display', () async {
    model.pi.platformAdditions['display_scale'] = true;
    final reply = Completer<DisplayScaleState>();
    var queries = 0;
    final target = DisplaySettingsTarget(session, (_, percent, __, ___) async {
      expect(percent, 0);
      if (++queries == 2) {
        throw const DisplayScaleError('changing', code: 'snapshot_changed');
      }
      return queries == 1 ? scaleState(100, 'before') : await reply.future;
    });
    await target.requestScale(0, '');
    var sends = 0;
    final operation =
        target.applyResolution(() => sends++, resolution: (2560, 1440));
    final rejected = expectLater(operation, throwsA(isA<DisplayScaleError>()));
    await Future<void>.delayed(Duration.zero);
    reply.complete(scaleState(100, 'new-mode',
        resolution: (2560, 1440), identity: 'replacement'));
    await rejected;
    expect(sends, 1);
    expect(queries, 3);
  });

  testWidgets(
      'combined Apply confirms the native mode across capture refreshes',
      (tester) async {
    model.pi.resolutions
        .addAll([Resolution(1920, 1080), Resolution(2560, 1440)]);
    final requests = <double>[];
    var currentScale = 100.0;
    var token = 'before';
    var resolution = (1920, 1080);
    var reads = 0;
    final target = DisplaySettingsTarget(session, (_, percent, __, ___) async {
      requests.add(percent);
      if (percent == 0 && ++reads == 3) {
        resolution = (2560, 1440);
        token = 'new-mode';
        model.pi.displays.value = [Display()];
      }
      if (percent != 0) {
        expect(resolution, (2560, 1440));
        expect(token, 'new-mode');
        currentScale = percent;
        token = 'after';
        model.pi.displays.value = [Display()];
      }
      return scaleState(currentScale, token, resolution: resolution);
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
                      return await target.applyResolution(
                          () => modes.add((width, height, scale)),
                          resolution: (width, height));
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
    expect(requests.where((percent) => percent != 0), [125]);
    expect(modes, [(2560, 1440, 1)]);
    expect(closed, isTrue);
    expect(tester.takeException(), isNull);
  });
}
