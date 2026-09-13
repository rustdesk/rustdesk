import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/desktop_render_texture.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/image.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:uuid/uuid.dart';
import 'package:vector_math/vector_math.dart' show Vector2;

const _viewport = Size(200, 160);
const _hotspot = Offset(4, 9);
const _scrollDeltas = [80.0, -20.0, 1000.0, 1000.0, -1000.0, -1000.0, 80.0, 0.0];

class _Peer extends Fake implements FfiModel {
  _Peer(ui.Image frame)
      : rect = Rect.fromLTWH(
            0, 0, frame.width.toDouble(), frame.height.toDouble()) {
    pi.displays.add(Display()
      ..width = frame.width
      ..height = frame.height);
  }
  @override
  final pi = PeerInfo();
  @override
  final Rect rect;
  @override
  bool get isPeerLinux => false;
}

class _Image extends ImageModel {
  _Image(FFI ffi, this.image, this.useTextureRender)
      : super(WeakReference(ffi));
  @override
  final ui.Image image;
  @override
  final bool useTextureRender;
}

class _Texture extends Fake implements TextureModel {
  @override
  RxInt getTextureId(int display) => 0.obs;
}

class _ScrollCanvas extends CanvasModel {
  _ScrollCanvas(FFI ffi, String style, this.scrollStyle)
      : viewStyle = ViewStyle(
            style: style,
            width: _viewport.width,
            height: _viewport.height,
            displayWidth: ffi.ffiModel.rect!.width.toInt(),
            displayHeight: ffi.ffiModel.rect!.height.toInt()),
        super(WeakReference(ffi)) {
    resetOffset();
    imageOverflow.value = true;
  }
  Size viewport = _viewport;
  double imageScale = 1;
  @override
  Size get size => viewport;
  @override
  double get scale => imageScale;
  @override
  double get x => (size.width - getDisplayWidth() * scale) / 2;
  @override
  double get y => (size.height - getDisplayHeight() * scale) / 2;
  @override
  final ScrollStyle scrollStyle;
  @override
  final ViewStyle viewStyle;

  void relayout(Size viewport, double dpr) {
    this.viewport = viewport;
    imageScale = 1 / dpr;
    imageOverflow.value = viewport.width < getDisplayWidth() * scale ||
        viewport.height < getDisplayHeight() * scale;
    setScrollPercent(0, 0);
    notifyListeners();
  }
}

class _Cursor extends CursorModel {
  _Cursor(FFI ffi, this.image, this.position) : super(WeakReference(ffi));
  final Offset position;
  @override
  final ui.Image image;
  @override
  double get hotx => _hotspot.dx;
  @override
  double get hoty => _hotspot.dy;
  @override
  double get x => position.dx;
  @override
  double get y => position.dy;
}

class _FFI extends Fake implements FFI {
  _FFI(ui.Image frame, ui.Image cursor,
      {required bool texture, required (String, ScrollStyle) style, required Offset position})
      : ffiModel = _Peer(frame) {
    imageModel = _Image(this, frame, texture);
    canvasModel = _ScrollCanvas(this, style.$1, style.$2);
    cursorModel = _Cursor(this, cursor, position);
  }
  @override
  final sessionId = Uuid().v4obj();
  @override
  final _Peer ffiModel;
  @override
  late final ImageModel imageModel;
  @override
  late final CanvasModel canvasModel;
  @override
  late final CursorModel cursorModel;
  @override
  final textureModel = _Texture();
}

class _Draw extends Fake implements Canvas {
  double factor = 1;
  Offset? position;
  @override
  void scale(double sx, [double? sy]) => factor *= sx;
  @override
  void drawImage(ui.Image image, Offset offset, Paint paint) =>
      position = offset * factor;
}

void main() {
  for (final style in [(kRemoteViewStyleOriginal, ScrollStyle.scrolledge),
    (kRemoteViewStyleCustom, ScrollStyle.scrolledge),
    (kRemoteViewStyleCustom, ScrollStyle.scrollbar)]) {
    for (final texture in [false, true]) {
      for (final (frame, dpr) in [
        (const Size(199, 320), 1.0),
        (const Size(198, 320), 2.0),
        (const Size(400, 159), 2.0),
        (const Size(400, 158), 1.0),
      ]) {
        testWidgets(
            'Scroll $style texture=$texture frame=$frame DPR=$dpr',
            (tester) => tester
                .runAsync(() => _check(tester, (style, texture, frame, dpr))));
      }
      for (final refreshBeforeLayout in [true, false]) {
        testWidgets(
            'Scroll relayout $style texture=$texture early=$refreshBeforeLayout',
            (tester) => tester.runAsync(() =>
                _checkRelayout(tester, (style, texture, refreshBeforeLayout))));
      }
    }
  }
}

Future<void> _check(
    WidgetTester tester, ((String, ScrollStyle), bool, Size, double) testCase) async {
  final (style, texture, frame, dpr) = testCase;
  tester.view.devicePixelRatio = dpr;
  tester.view.physicalSize = _viewport * dpr;
  addTearDown(tester.view.reset);
  final vertical = frame.height > _viewport.height;
  final pointer =
      vertical ? const Offset(50.25, 200.75) : const Offset(240.25, 50.75);
  final video = await createTestImage(
      width: frame.width.toInt(), height: frame.height.toInt());
  final cursor = await createTestImage(width: 48, height: 64);
  final ffi =
      _FFI(video, cursor, texture: texture, style: style, position: pointer);
  addTearDown(() {
    ffi.canvasModel.scrollHorizontal.dispose();
    ffi.canvasModel.scrollVertical.dispose();
    ffi.canvasModel.dispose();
    ffi.imageModel.dispose();
    ffi.cursorModel.dispose();
    video.dispose();
    cursor.dispose();
  });
  await _mount(tester, ffi, dpr);
  final videoWidget = texture ? find.byType(Texture) : _paintOf(video);
  final scrolling = vertical
      ? ffi.canvasModel.scrollVertical
      : ffi.canvasModel.scrollHorizontal;
  for (final distance in _scrollDeltas) {
    // Drive the real scroll controllers; canvas pan offsets are not the video origin.
    ffi.canvasModel.performEdgeScroll(
        vertical ? Vector2(0, distance) : Vector2(distance, 0));
    expect(vertical ? ffi.canvasModel.scrollY : ffi.canvasModel.scrollX,
        closeTo(scrolling.offset / (vertical ? frame.height : frame.width), 1e-9));
    await tester.pump();
    _expectAlignment(tester, ffi, videoWidget);
  }
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> _checkRelayout(
    WidgetTester tester, ((String, ScrollStyle), bool, bool) testCase) async {
  final (style, texture, refreshBeforeLayout) = testCase;
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = _viewport;
  addTearDown(tester.view.reset);
  final video = await createTestImage(width: 400, height: 320);
  final cursor = await createTestImage(width: 48, height: 64);
  final ffi = _FFI(video, cursor,
      texture: texture, style: style, position: const Offset(150.25, 120.75));
  final canvas = ffi.canvasModel as _ScrollCanvas;
  addTearDown(() {
    canvas.scrollHorizontal.dispose();
    canvas.scrollVertical.dispose();
    canvas.dispose();
    ffi.imageModel.dispose();
    ffi.cursorModel.dispose();
    video.dispose();
    cursor.dispose();
  });
  await _mount(tester, ffi, 1);
  canvas.performEdgeScroll(Vector2(20, 20));
  await tester.pump();
  final videoWidget = texture ? find.byType(Texture) : _paintOf(video);
  _expectAlignment(tester, ffi, videoWidget);
  for (final (viewport, dpr) in [
    (const Size(160, 120), 2.0),
    (const Size(195, 155), 2.0), // Clamp both existing scroll positions.
    (const Size(240, 155), 2.0), // Detach the horizontal scroll controller.
    (const Size(240, 200), 2.0), // No scrolling remains.
    (_viewport, 1.0),
  ]) {
    tester.view.devicePixelRatio = dpr;
    tester.view.physicalSize = viewport * dpr;
    canvas.relayout(viewport, dpr);
    // A settings refresh may run before layout or after the first cursor build.
    if (refreshBeforeLayout) {
      canvas.updateScrollPercent();
    } else {
      tester.binding.addPostFrameCallback((_) => canvas.updateScrollPercent());
    }
    await _mount(tester, ffi, dpr);
    await tester.pumpAndSettle(const Duration(milliseconds: 16),
        EnginePhase.sendSemanticsUpdate, const Duration(seconds: 1));
    _expectAlignment(tester, ffi, videoWidget);
  }
  await tester.pumpWidget(const SizedBox.shrink());
}

void _expectAlignment(WidgetTester tester, FFI ffi, Finder videoWidget) {
  final cursor = ffi.cursorModel;
  final cursorWidget = _paintOf(cursor.image!);
  final painter =
      tester.widget<CustomPaint>(cursorWidget).painter! as ImagePainter;
  final output = _Draw();
  painter.paint(output, ffi.canvasModel.size);
  final hotspot = tester.getTopLeft(cursorWidget) +
      output.position! +
      _hotspot * output.factor;
  var videoOrigin = tester.getTopLeft(videoWidget);
  final video = tester.widget(videoWidget);
  if (video is CustomPaint) {
    final drawnVideo = _Draw();
    video.painter!.paint(drawnVideo, ffi.canvasModel.size);
    videoOrigin += drawnVideo.position!;
  }
  final target =
      videoOrigin + Offset(cursor.x, cursor.y) * ffi.canvasModel.scale;
  expect(hotspot.dx, closeTo(target.dx, 1e-9));
  expect(hotspot.dy, closeTo(target.dy, 1e-9));
}

Finder _paintOf(ui.Image image) => find.byWidgetPredicate((widget) =>
    widget is CustomPaint &&
    widget.painter is ImagePainter &&
    identical((widget.painter as ImagePainter).image, image));

Future<void> _mount(WidgetTester tester, FFI ffi, double dpr) =>
    tester.pumpWidget(Directionality(
        textDirection: TextDirection.ltr,
        child: MediaQuery(
            data: MediaQueryData(devicePixelRatio: dpr),
            child: MultiProvider(
                providers: [
                  ChangeNotifierProvider<ImageModel>.value(
                      value: ffi.imageModel),
                  ChangeNotifierProvider<CanvasModel>.value(
                      value: ffi.canvasModel),
                  ChangeNotifierProvider<CursorModel>.value(
                      value: ffi.cursorModel),
                ],
                child: Stack(fit: StackFit.expand, children: [
                  ImagePaint(
                      ffi: ffi,
                      id: 'scroll-edge-test',
                      zoomCursor: false.obs,
                      cursorOverImage: false.obs,
                      keyboardEnabled: true.obs,
                      remoteCursorMoved: false.obs),
                  CursorPaint(id: 'scroll-edge-test', zoomCursor: false.obs),
                ])))));
