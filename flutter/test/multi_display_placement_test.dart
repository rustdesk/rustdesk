import 'package:flutter_hbb/common/multi_display_placement.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const left = DisplayRect(0, 0, 1920, 1080);
  const right = DisplayRect(1920, 0, 3840, 1080);

  test('unsaved placement keeps enumeration order', () {
    final placed = assignRemoteDisplaysToLocalScreens(
      remoteCount: 2,
      localScreens: const [left, right],
      savedOrigins: {},
    );
    expect(placed, [left, right]);
  });

  test('saved origins put each remote display back on its screen', () {
    final placed = assignRemoteDisplaysToLocalScreens(
      remoteCount: 2,
      localScreens: const [left, right],
      savedOrigins: {
        0: ScreenOrigin(right.left, right.top),
        1: ScreenOrigin(left.left, left.top),
      },
    );
    expect(placed, [right, left]);
  });

  test(
    'a missing screen falls back without giving two displays one screen',
    () {
      final placed = assignRemoteDisplaysToLocalScreens(
        remoteCount: 2,
        localScreens: const [left, right],
        savedOrigins: {0: ScreenOrigin(5000, 0), 1: ScreenOrigin(5000, 0)},
      );
      expect(placed, [left, right]);
    },
  );

  test('the first display keeps a screen both displays claim', () {
    final placed = assignRemoteDisplaysToLocalScreens(
      remoteCount: 2,
      localScreens: const [left, right],
      savedOrigins: {
        0: ScreenOrigin(right.left, right.top),
        1: ScreenOrigin(right.left, right.top),
      },
    );
    expect(placed, [right, left]);
  });

  test('a left fullscreen frame does not count as covering the right screen',
      () {
    const leftFrame = DisplayRect(0, 0, 1920, 1080);
    expect(fullscreenFrameAlreadyOnScreen(leftFrame, left), isTrue);
    expect(fullscreenFrameAlreadyOnScreen(leftFrame, right), isFalse);
    expect(
      fullscreenFrameAlreadyOnScreen(const DisplayRect(1920, 0, 3840, 1080), right),
      isTrue,
    );
  });

  test('screen origin round trip and window overlap', () {
    expect(
      decodeScreenOrigin(encodeScreenOrigin(1920, 0)),
      const ScreenOrigin(1920, 0),
    );
    expect(decodeScreenOrigin('nope'), isNull);
    expect(
      screenForWindow(const [
        left,
        right,
      ], const DisplayRect(2000, 10, 2800, 610)),
      right,
    );
    expect(
      shownRemoteDisplay(
        peerReady: false,
        currentDisplay: 0,
        initialDisplay: 1,
      ),
      1,
    );
    expect(
      shownRemoteDisplay(peerReady: true, currentDisplay: 0, initialDisplay: 1),
      0,
    );
    expect(
      shownRemoteDisplay(
          peerReady: true, currentDisplay: -1, initialDisplay: 1),
      isNull,
    );
    expect(
      shownRemoteDisplay(
          peerReady: false, currentDisplay: 0, initialDisplay: null),
      isNull,
    );
    expect(parseRemoteDisplayEntry('277161259:1')?.display, 1);
    expect(parseRemoteDisplayEntry('277161259:1')?.peerId, '277161259');
    expect(parseRemoteDisplayEntry('bad'), isNull);
  });
}
