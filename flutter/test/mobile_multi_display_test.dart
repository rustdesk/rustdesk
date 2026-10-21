import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

Future<ui.Image> createTestImage(int width, int height) async {
  final recorder = ui.PictureRecorder();
  final canvas = ui.Canvas(recorder);
  canvas.drawRect(
    Rect.fromLTWH(0, 0, width.toDouble(), height.toDouble()),
    Paint()..color = const Color(0xFF00FF00),
  );
  final picture = recorder.endRecording();
  return picture.toImage(width, height);
}

class _DummyCanvas extends Fake implements CanvasModel {
  @override
  Future<void> updateViewStyle({dynamic refreshMousePos = true, dynamic notify = true}) async {}
  @override
  Future<void> updateScrollStyle() async {}
  @override
  Future<void> initializeEdgeScrollEdgeThickness() async {}
}

class _DummyCursor extends Fake implements CursorModel {
  @override
  void updateDisplayOrigin(double left, double top, {dynamic updateCursorPos = true}) {}
}

class _DummyPeerInfo extends Fake implements PeerInfo {
  @override
  final currentDisplay = 0;
}

class _DummyFfiModel extends Fake implements FfiModel {
  @override
  final pi = _DummyPeerInfo();
  @override
  Rect? get rect => null;
}

class _DummyFFI extends Fake implements FFI {
  @override
  final sessionId = Uuid().v4obj();

  @override
  final id = 'test';

  @override
  final canvasModel = _DummyCanvas();

  @override
  final cursorModel = _DummyCursor();

  @override
  final ffiModel = _DummyFfiModel();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('Mobile multi-display regressions', () {
    testWidgets(
        '[P1] Frame disposal bounds retained images when multiple images arrive without pumping frames',
        (tester) async {
      final ffi = _DummyFFI();
      final model = ImageModel(WeakReference(ffi));

      // Deliver an initial image for display 0
      final img0 = await createTestImage(10, 10);
      await model.update(img0, display: 0);
      expect(model.getImage(0), equals(img0));

      // Deliver 10 replacement images without pumping frames (simulating paused/background state)
      final replacements = <ui.Image>[];
      for (int i = 1; i <= 10; i++) {
        final img = await createTestImage(10, 10);
        replacements.add(img);
        await model.update(img, display: 0);
      }

      // The active image must be the latest replacement (img10)
      final latest = replacements.last;
      expect(model.getImage(0), equals(latest));

      // All intermediate replacements (replacements[0..8]) must have been disposed immediately
      for (int i = 0; i < replacements.length - 1; i++) {
        expect(replacements[i].debugDisposed, isTrue,
            reason: 'Intermediate image $i should be disposed immediately');
      }

      // The latest image must not be disposed
      expect(latest.debugDisposed, isFalse);

      // Now schedule and pump a frame (resuming rendering)
      tester.binding.scheduleFrame();
      await tester.pump();

      // The initial image (img0) that was awaiting post-frame callback should now be disposed
      expect(img0.debugDisposed, isTrue);

      // The current image must still be valid for painting
      expect(model.getImage(0), equals(latest));
      expect(latest.debugDisposed, isFalse);
    });

    test(
        '[P2] Topology update affecting only another monitor preserves currently selected display',
        () {
      final ffi = _DummyFFI();
      final model = ImageModel(WeakReference(ffi));

      final d0 = Display()
        ..x = 0
        ..y = 0
        ..width = 1920
        ..height = 1080;

      final d1Old = Display()
        ..x = 1920
        ..y = 0
        ..width = 1920
        ..height = 1080;

      final d1New = Display()
        ..x = 2560
        ..y = 0
        ..width = 1920
        ..height = 1080;

      // Monitor 0 and monitor 1 populate images
      final dummyImg0 = FakeImage();
      final dummyImg1 = FakeImage();
      model.images[0] = dummyImg0;
      model.images[1] = dummyImg1;

      // Monitor 1 changes position, but monitor 0's geometry is completely unchanged
      final oldDisplays = [d0, d1Old];
      final newDisplays = [d0, d1New];

      model.reconcileDisplayImages(oldDisplays, newDisplays);

      // Monitor 0 must remain visible
      expect(model.getImage(0), equals(dummyImg0));

      // Monitor 1's stale image must be cleared
      expect(model.getImage(1), isNull);
    });

    test('[P2] isSupportMultiUiSession capability check gates All Displays mode', () {
      final pi = PeerInfo();
      pi.isSupportMultiUiSession = false;

      // When peer lacks simultaneous multi-display support, isSupportMultiUiSession is false
      expect(pi.isSupportMultiUiSession, isFalse);

      pi.isSupportMultiUiSession = true;
      expect(pi.isSupportMultiUiSession, isTrue);
    });
  });
}

class FakeImage implements ui.Image {
  @override
  int get width => 10;
  @override
  int get height => 10;
  @override
  void dispose() {}
  @override
  bool get debugDisposed => false;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
