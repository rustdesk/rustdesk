import 'package:flutter_hbb/utils/display_resolution.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('aspect ratio recognition handles orientation and pixel rounding', () {
    expect(displayAspectRatio(800, 1280), (10, 16));
    expect(displayAspectRatio(1366, 768), (16, 9));
    expect(displayAspectRatio(1441, 901), (16, 10));
    expect(displayAspectRatio(1440, 3120), isNull);
    expect(displayAspectRatio(0, 1080), isNull);
  });

  test('linked dimensions preserve the edited axis and round only the other',
      () {
    expect(linkedDisplayDimension(1366, (16, 9), widthChanged: true), 768);
    expect(linkedDisplayDimension(768, (16, 9), widthChanged: false), 1365);
    expect(linkedDisplayDimension(0, (16, 9), widthChanged: true), isNull);
  });

  (int, int)? fit((int, int) size,
          {int scale = 1, List<(int, int)>? supported}) =>
      fitDisplayResolution(
          localSize: size,
          minDimension: 320,
          maxDimension: 4096,
          scale: scale,
          allowArbitrarySize: supported == null,
          supported: supported ?? []);

  test('virtual fitting preserves size or aspect ratio within native limits',
      () {
    expect(fit((3048, 2032), scale: 2), (3048, 2032));
    expect(fit((5120, 2880), scale: 2), (4096, 2304));
    expect(fit((2880, 5120), scale: 2), (2304, 4096));
    expect(fit((160, 90), scale: 2), (568, 320));
    expect(fit((2559, 1601), scale: 2), (2560, 1602));
    expect(fit((1081, 2401), scale: 2), (1082, 2402));
  });

  test('physical fitting prefers aspect ratio then nearest area', () {
    const modes = [(1280, 720), (1920, 1080), (2560, 1440), (2560, 1600)];
    expect(fit((1920, 1080), supported: modes), (1920, 1080));
    expect(fit((2340, 1080), supported: modes), (1920, 1080));
    expect(fit((3000, 2000), supported: modes), (2560, 1600));
    expect(fit((1440, 3120), supported: [(1920, 1080), (1080, 1920)]),
        (1080, 1920));
    expect(fit((1440, 3120), supported: [(1920, 1080)]), (1920, 1080));
    expect(fit((2340, 1080), supported: modes.reversed.toList()), (1920, 1080));
  });

  test('invalid local sizes and unusable modes do not produce a match', () {
    expect(fit((0, 1080)), isNull);
    expect(fit((-1, 1080)), isNull);
    expect(fit((1920, 1080), scale: 0), isNull);
    expect(fit((100, 20000)), isNull);
    expect(fit((1920, 1080), supported: []), isNull);
    expect(fit((5120, 2880), supported: [(5120, 2880), (0, 0)]), isNull);
    expect(fit((1920, 1080), scale: 2, supported: [(2559, 1600)]), isNull);
  });
}
