import 'dart:convert';

import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/virtual_display.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

final _sessionId = UuidValue('00000000-0000-0000-0000-000000000000');

class _FakeCanvas implements CanvasModel {
  @override
  dynamic tryUpdateScrollStyle(Duration duration, String? style) {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _FakeFFI implements FFI {
  @override
  UuidValue get sessionId => _sessionId;

  @override
  final CanvasModel canvasModel = _FakeCanvas();

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _DisplayModel extends FfiModel {
  _DisplayModel(FFI ffi) : super(WeakReference(ffi));

  @override
  Future<void> updateCurDisplay(UuidValue sessionId,
      {updateCursorPos = false}) async {}
}

void main() {
  test(
      'logical requests convert current dimensions and bounds using native scale',
      () {
    final hidpi = virtualDisplayResolutionDimensions((320, 320, 2, 42),
        outputPixels: false);
    expect(hidpi,
        (width: 160, height: 160, minDimension: 160, maxDimension: 2048));
    expect(
        virtualDisplayResolutionDimensions((2560, 1600, 2, 42),
            outputPixels: false),
        (width: 1280, height: 800, minDimension: 160, maxDimension: 2048));
    expect(
        virtualDisplayResolutionDimensions((2560, 1600, 2, 42),
            outputPixels: true),
        (width: 2560, height: 1600, minDimension: 320, maxDimension: 4096));
    expect(
        virtualDisplayResolutionDimensions((1920, 1080, 1, 42),
            outputPixels: false),
        (width: 1920, height: 1080, minDimension: 320, maxDimension: 4096));
  });

  test('missing or malformed native state is never inferred from capture', () {
    for (final modes in [null, [], 'bad']) {
      expect(nativeVirtualDisplayMode({kMacOSVirtualDisplayModes: modes}, 0),
          isNull);
    }
    for (final mode in [
      null,
      [2560, 1600, 2],
      [2560, 1600, 2, 0],
      [2560, 1600, 2, -1],
      [2560, 1600, 2, 0x100000000],
      [2560, 1600, 2, '42'],
      [2560, 1600],
      ['2560', 1600, 2, 42],
      [2560.0, 1600, 2, 42],
      [2560, 1600, 0, 42],
      [2560, 1600, 3, 42],
      [2559, 1600, 2, 42],
      [0, 1600, 1, 42],
      [5120, 2880, 2, 42],
    ]) {
      expect(
          nativeVirtualDisplayMode({
            kMacOSVirtualDisplayModes: {'0': mode}
          }, 0),
          isNull);
    }
    expect(nativeVirtualDisplayMode({}, 0), isNull);
  });

  test(
      'mode-only updates change logical bounds even with identical capture geometry',
      () async {
    final ffi = _FakeFFI();
    final model = _DisplayModel(ffi);
    model.pi.currentDisplay = 0;
    final displayEvent = {
      'displays': jsonEncode([
        {'width': 1280, 'height': 800}
      ]),
    };
    await model.handleSyncPeerInfo(displayEvent, _sessionId, 'peer');
    await model.handlePlatformAdditions({
      'platform_additions': jsonEncode({
        kMacOSVirtualDisplayModes: {
          '0': [2560, 1600, 2, 42]
        },
      }),
    }, _sessionId, 'peer');
    final before = nativeVirtualDisplayMode(model.pi.platformAdditions, 0)!;
    final beforeBounds =
        virtualDisplayResolutionDimensions(before, outputPixels: false);
    await model.handleSyncPeerInfo(displayEvent, _sessionId, 'peer');
    await model.handlePlatformAdditions({
      'platform_additions': jsonEncode({
        kMacOSVirtualDisplayModes: {
          '0': [1280, 800, 1, 42]
        },
      }),
    }, _sessionId, 'peer');
    final after = nativeVirtualDisplayMode(model.pi.platformAdditions, 0)!;
    final afterBounds =
        virtualDisplayResolutionDimensions(after, outputPixels: false);
    expect(after, isNot(before));
    expect(after.$4, before.$4);
    expect(model.pi.displays.single.width, 1280);
    expect(model.pi.displays.single.height, 800);
    expect((beforeBounds.width, beforeBounds.height),
        (afterBounds.width, afterBounds.height));
    expect(beforeBounds.maxDimension, 2048);
    expect(afterBounds.maxDimension, 4096);
    model.dispose();
  });

  test('display replacement invalidates native modes until matching update',
      () async {
    final ffi = _FakeFFI();
    final model = _DisplayModel(ffi);
    model.pi.currentDisplay = 0;
    model.pi.platformAdditions.addAll({
      'virtual_display_native_scale': [1, 2],
      kMacOSVirtualDisplayModes: {
        '0': [2560, 1600, 2, 42]
      },
    });

    await model.handleSyncPeerInfo({
      'displays': jsonEncode([
        {'width': 1920, 'height': 1080}
      ]),
    }, _sessionId, 'peer');
    expect(nativeVirtualDisplayMode(model.pi.platformAdditions, 0), isNull);
    expect(model.pi.platformAdditions.containsKey(kMacOSVirtualDisplayModes),
        isTrue);
    expect(model.pi.platformAdditions['virtual_display_native_scale'], [1, 2]);
    final cached =
        jsonDecode(model.cachedPeerData.peerInfo['platform_additions'])
            as Map<String, dynamic>;
    expect(nativeVirtualDisplayMode(cached, 0), isNull);

    await model.handlePlatformAdditions({
      'platform_additions': jsonEncode({
        kMacOSVirtualDisplayModes: {
          '0': [1920, 1080, 1, 43]
        },
      }),
    }, _sessionId, 'peer');
    expect(nativeVirtualDisplayMode(model.pi.platformAdditions, 0),
        (1920, 1080, 1, 43));
    model.dispose();
  });
}
