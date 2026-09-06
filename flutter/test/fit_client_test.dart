import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/fit_client.dart';

void main() {
  const laptopModes = [
    HostMode(2880, 1800),
    HostMode(2560, 1600),
    HostMode(1920, 1200),
    HostMode(1920, 1080),
    HostMode(1600, 900),
    HostMode(1280, 800),
    HostMode(1280, 720),
    HostMode(1024, 768),
  ];

  const iphone = FitClientViewport(
    logicalWidth: 393,
    logicalHeight: 852,
    devicePixelRatio: 3,
  );

  test('empty host mode list returns null', () {
    expect(
      pickFitClientMode(viewport: iphone, hostModes: const []),
      isNull,
    );
  });

  test('iPhone portrait picks a small landscape desktop, not 2880x1800', () {
    final picked = pickFitClientMode(
      viewport: iphone,
      hostModes: laptopModes,
    );
    expect(picked, isNotNull);
    expect(picked!.width, lessThanOrEqualTo(1280));
    expect(picked.height, lessThanOrEqualTo(800));
    expect(picked.width, greaterThanOrEqualTo(1280));
    expect(picked.height, greaterThanOrEqualTo(720));
  });

  test('iPhone landscape picks the same readable desktop', () {
    const landscape = FitClientViewport(
      logicalWidth: 852,
      logicalHeight: 393,
      devicePixelRatio: 3,
    );
    final portrait = pickFitClientMode(
      viewport: iphone,
      hostModes: laptopModes,
    );
    final rotated = pickFitClientMode(
      viewport: landscape,
      hostModes: laptopModes,
    );
    expect(portrait, rotated);
  });

  test('chosen mode is more readable than native 2880x1800', () {
    final picked = pickFitClientMode(
      viewport: iphone,
      hostModes: laptopModes,
    )!;
    // Physical iPhone landscape after the portrait→landscape remap.
    const clientW = 852.0 * 3;
    const clientH = 393.0 * 3;
    final native = adaptiveFitScale(
      clientW: clientW,
      clientH: clientH,
      hostW: 2880,
      hostH: 1800,
    );
    final fitted = adaptiveFitScale(
      clientW: clientW,
      clientH: clientH,
      hostW: picked.width.toDouble(),
      hostH: picked.height.toDouble(),
    );
    expect(fitted, greaterThan(native));
    expect(fitted, greaterThan(1.0));
  });

  test('does not invent a mode missing from the host list', () {
    final picked = pickFitClientMode(
      viewport: iphone,
      hostModes: const [HostMode(1920, 1080), HostMode(1280, 720)],
    );
    expect(picked, anyOf(const HostMode(1920, 1080), const HostMode(1280, 720)));
  });

  test('falls back to the largest mode when none meet the desktop minimum', () {
    final picked = pickFitClientMode(
      viewport: iphone,
      hostModes: const [HostMode(800, 600), HostMode(640, 480)],
    );
    expect(picked, const HostMode(800, 600));
  });

  test('deduplicates identical width x height entries', () {
    final picked = pickFitClientMode(
      viewport: iphone,
      hostModes: const [
        HostMode(1280, 800),
        HostMode(1280, 800),
        HostMode(2880, 1800),
      ],
    );
    expect(picked, const HostMode(1280, 800));
  });
}
