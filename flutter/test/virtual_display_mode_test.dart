import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/utils/virtual_display.dart';

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
    for (final modes in [
      null,
      {
        '0': [2560, 1600, 2]
      },
      {
        '0': [2560, 1600, 2, 0]
      },
      {
        '0': [2560, 1600, 2, -1]
      },
      {
        '0': [2560, 1600, 2, 0x100000000]
      },
      {
        '0': [2560, 1600, 2, '42']
      },
      [],
      'bad',
      {'0': null},
      {
        '0': [2560, 1600]
      },
      {
        '0': ['2560', 1600, 2, 42]
      },
      {
        '0': [2560.0, 1600, 2, 42]
      },
      {
        '0': [2560, 1600, 0, 42]
      },
      {
        '0': [2560, 1600, 3, 42]
      },
      {
        '0': [2559, 1600, 2, 42]
      },
      {
        '0': [0, 1600, 1, 42]
      },
      {
        '0': [5120, 2880, 2, 42]
      }
    ]) {
      expect(nativeVirtualDisplayMode({kMacOSVirtualDisplayModes: modes}, 0),
          isNull);
    }
    expect(nativeVirtualDisplayMode({}, 0), isNull);
  });
}
