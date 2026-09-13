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
  _ScrollCanvas(FFI ffi, String style)
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
  @override
  Size get size => _viewport;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrolledge;
  @override
  final ViewStyle viewStyle;
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
      {required bool texture, required String style, required Offset position})
      : ffiModel = _Peer(frame) {
    imageModel = _Image(this, frame, texture);
    canvasModel = _ScrollCanvas(this, style);
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
  for (final style in [kRemoteViewStyleOriginal, kRemoteViewStyleCustom]) {
    for (final texture in [false, true]) {
      for (final (frame, dpr) in [
        (const Size(199, 320), 1.0),
        (const Size(198, 320), 2.0),
        (const Size(400, 159), 2.0),
        (const Size(400, 158), 1.0),
      ]) {
        testWidgets(
            'ScrollEdge $style texture=$texture frame=$frame DPR=$dpr',
            (tester) => tester
                .runAsync(() => _check(tester, (style, texture, frame, dpr))));
      }
    }
  }
}

Future<void> _check(
    WidgetTester tester, (String, bool, Size, double) testCase) async {
  final (style, texture, frame, dpr) = testCase;
  tester.view.devicePixelRatio = dpr;
  tester.view.physicalSize = _viewport * dpr;
  addTearDown(tester.view.reset);
  final vertical = frame.height > _viewport.height;
  final pointer =
      vertical ? const Offset(50.25, 130.75) : const Offset(130.25, 50.75);
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
  for (final distance in [80.0, -20.0]) {
    // Drive the real scroll controllers; canvas pan offsets are not the video origin.
    ffi.canvasModel.performEdgeScroll(
        vertical ? Vector2(0, distance) : Vector2(distance, 0));
    await tester.pump();
    final painter =
        tester.widget<CustomPaint>(_paintOf(cursor)).painter! as ImagePainter;
    final output = _Draw();
    painter.paint(output, _viewport);
    final hotspot = tester.getTopLeft(_paintOf(cursor)) +
        output.position! +
        _hotspot * output.factor;
    final target = tester.getTopLeft(videoWidget) + pointer;
    expect(hotspot.dx, closeTo(target.dx, 1e-9));
    expect(hotspot.dy, closeTo(target.dy, 1e-9));
  }
  await tester.pumpWidget(const SizedBox.shrink());
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
