import 'dart:io';
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

const _hotspot = Offset(4, 9);
const _remotePosition = Offset(100.25, 80.75);
const _canvasOffset = Offset(15.125, -10.25);
const _viewport = Size(200, 160);

class _CursorModel extends ChangeNotifier implements CursorModel {
  _CursorModel(this.image, this.position, this.density);

  final Offset position;
  final double density;
  @override
  CursorData get cache => _Density(density);

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

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Density extends Fake implements CursorData {
  _Density(this.pixelRatio);
  @override
  final double pixelRatio;
}

class _ImageModel extends ChangeNotifier implements ImageModel {
  _ImageModel(this.useTextureRender);

  @override
  final bool useTextureRender;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _TextureModel extends Fake implements TextureModel {
  @override
  RxInt getTextureId(int display) => 0.obs;
}

class _Display extends Display {
  _Display(this.scale, double left) {
    x = left;
    width = 3840;
    height = 2160;
  }

  @override
  final double scale;
}

class _Peer extends Fake implements FfiModel {
  @override
  final pi = PeerInfo()
    ..displays.add(Display()
      ..width = _viewport.width.toInt()
      ..height = _viewport.height.toInt());
  @override
  bool isPeerLinux = false;
  @override
  Rect rect = Offset.zero & _viewport;
}

class _FFI extends Fake implements FFI {
  _FFI(bool useTexture) : imageModel = _ImageModel(useTexture);

  @override
  final ImageModel imageModel;
  @override
  final _Peer ffiModel = _Peer();
  @override
  final textureModel = _TextureModel();
  @override
  late CanvasModel canvasModel;
}

class _CanvasModel extends ChangeNotifier implements CanvasModel {
  _CanvasModel(String style, this.scale, bool useTexture)
      : _ffi = _FFI(useTexture),
        viewStyle = ViewStyle(
          style: style,
          width: _viewport.width,
          height: _viewport.height,
          displayWidth: _viewport.width.toInt(),
          displayHeight: _viewport.height.toInt(),
        ) {
    _ffi.canvasModel = this;
  }

  final _FFI _ffi;
  @override
  WeakReference<FFI> get parent => WeakReference(_ffi);
  @override
  final imageOverflow = false.obs;
  @override
  final ViewStyle viewStyle;
  @override
  double get x => _canvasOffset.dx;
  @override
  double get y => _canvasOffset.dy;
  @override
  final double scale;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollauto;
  @override
  Size get size => _viewport;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _ScrollbarCanvasModel extends _CanvasModel {
  _ScrollbarCanvasModel(String style, {Size? frame, double scale = 2})
      : super(style, scale, true) {
    imageOverflow.value = true;
    if (frame != null) _ffi.ffiModel.rect = Offset.zero & frame;
  }

  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollbar;
  @override
  double get scrollX => _ffi.ffiModel.rect.width * scale > size.width ? 0.1 : 0;
  @override
  double get scrollY => _ffi.ffiModel.rect.height * scale > size.height ? 0.2 : 0;
}

class _Canvas extends Fake implements Canvas {
  double factor = 1;
  Offset? position;

  @override
  void scale(double sx, [double? sy]) => factor *= sx;

  @override
  void drawImage(ui.Image image, Offset offset, Paint paint) {
    position = offset * factor;
  }
}

Future<ImagePainter> _paintCursor(WidgetTester tester, CanvasModel canvas,
    {double dpr = 2, bool zoom = true, (int, int) source = (48, 64),
    Offset position = _remotePosition, double density = 0}) async {
  final image = (await tester
      .runAsync(() => createTestImage(width: source.$1, height: source.$2)))!;
  addTearDown(image.dispose);
  tester.view.devicePixelRatio = dpr;
  tester.view.physicalSize = _viewport * dpr;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(Directionality(textDirection: TextDirection.ltr,
    child: MediaQuery(
    data: MediaQueryData(devicePixelRatio: dpr),
    child: MultiProvider(
      providers: [
        ChangeNotifierProvider<ImageModel>.value(value: canvas.parent.target!.imageModel),
        ChangeNotifierProvider<CursorModel>(create: (_) => _CursorModel(image, position, density)),
        ChangeNotifierProvider<CanvasModel>(create: (_) => canvas),
      ],
      child: Stack(fit: StackFit.expand, children: [
        // Measure the video's origin instead of copying its rounding policy.
        if (canvas.parent.target!.imageModel.useTextureRender &&
            canvas.scrollStyle == ScrollStyle.scrollauto)
          ImagePaint(ffi: canvas.parent.target!, id: 'cursor-test',
              zoomCursor: zoom.obs, cursorOverImage: false.obs,
              keyboardEnabled: true.obs, remoteCursorMoved: false.obs),
        CursorPaint(id: 'cursor-test', zoomCursor: zoom.obs),
      ]),
    ),
  )));
  final painter = tester.widget<CustomPaint>(find.byType(CustomPaint)).painter!
      as ImagePainter;
  expect(painter.image, same(image));
  return painter;
}

void _linuxDisplayTests() {
  for (final (texture, displayScale, allDisplays) in [
    (false, 2.0, false), (true, 2.0, false), (true, 2.0, true),
    (false, 1.0, false), (true, 1.0, false),
  ]) {
    testWidgets('Linux scale=$displayScale texture=$texture all=$allDisplays',
        (tester) async {
      final canvas = _CanvasModel(kRemoteViewStyleCustom, 2, texture);
      final peer = canvas._ffi.ffiModel;
      peer.isPeerLinux = true;
      // The first output's physical extent overlaps the second in logical
      // coordinates. Selection must use logical extents and the union origin.
      peer.pi.displays.assignAll([
        if (displayScale > 1) _Display(4, -960), _Display(displayScale, 0),
      ]);
      peer.pi.currentDisplay = allDisplays ? kAllDisplayValue
          : peer.pi.displays.length - 1;
      peer.rect = Rect.fromLTWH(allDisplays ? -960 : 0, 0, 3840, 2160);
      final painter = await _paintCursor(tester, canvas,
          dpr: 1, zoom: false, source: (64, 64), density: 2,
          position: allDisplays ? _remotePosition + const Offset(960, 0)
              : _remotePosition);
      final pixelScale = 2 / displayScale;
      expect(painter.scale, pixelScale);
      final origin = texture ? tester.getTopLeft(find.byType(Texture).first)
          : Offset((_canvasOffset.dx / pixelScale).toInt() * pixelScale,
              (_canvasOffset.dy / pixelScale).toInt() * pixelScale);
      final position = allDisplays
          ? _remotePosition + const Offset(960, 0) : _remotePosition;
      expect((Offset(painter.x, painter.y) + _hotspot) * painter.scale,
          position * 2 + origin);
    });
  }
}

void main() {
  _linuxDisplayTests();
  final minimumScale = Platform.isWindows ? 1 / 3 : 2 / 3;
  for (final (style, zoom, dpr, source, canvasScale, scale, texture, density) in [
    (kRemoteViewStyleAdaptive, false, 2.0, (48, 64), 0.375, 0.375, true, 0.0),
    (kRemoteViewStyleOriginal, false, 2.0, (48, 64), 0.5, 0.5, true, 0.0),
    (kRemoteViewStyleCustom, false, 2.0, (48, 64), 0.25, 0.25, false, 0.0),
    (kRemoteViewStyleCustom, true, 2.0, (48, 64), 2.0, 2.0, false, 0.0),
    (kRemoteViewStyleAdaptive, false, 2.25, (48, 48), 0.375, 0.375, false, 0.0),
    (kRemoteViewStyleAdaptive, true, 2.0, (9, 18), 0.1, minimumScale, true, 0.0),
    (kRemoteViewStyleAdaptive, true, 1.0, (48, 64), 0.05, 0.1875, true, 2.0),
    (kRemoteViewStyleAdaptive, true, 2.0, (48, 64), 0.05, 0.1875, true, 2.0),
    (kRemoteViewStyleCustom, true, 1.0, (48, 64), 0.05, 0.1875, true, 2.0),
    (kRemoteViewStyleCustom, true, 2.0, (48, 64), 0.05, 0.1875, true, 2.0),
    (kRemoteViewStyleCustom, true, 2.0, (8, 10), 1.0, 1.2, true, 2.0),
    (kRemoteViewStyleOriginal, true, 1.0, (48, 64), 0.05, 0.25, true, 2.0),
    (kRemoteViewStyleOriginal, true, 2.0, (48, 64), 0.05,
        Platform.isWindows ? 0.125 : 0.25, true, 2.0),
  ]) {
    testWidgets(
        '$style zoom=$zoom dpr=$dpr source=$source texture=$texture density=$density keeps remote geometry',
        (tester) async {
      final painter = await _paintCursor(
          tester, _CanvasModel(style, canvasScale, texture),
          dpr: dpr, zoom: zoom, source: source, density: density);
      expect(painter.scale, scale);
      var imageOrigin = texture
          ? tester.getTopLeft(find.byType(Texture).first) : _canvasOffset;
      if (!texture) {
        final background = _Canvas();
        ImagePainter(
          image: painter.image,
          x: _canvasOffset.dx / canvasScale,
          y: _canvasOffset.dy / canvasScale,
          scale: canvasScale,
        ).paint(background, _viewport);
        imageOrigin = background.position!;
      }
      final target = _remotePosition * canvasScale + imageOrigin;
      final hotspot = (Offset(painter.x, painter.y) + _hotspot) * scale;
      expect(hotspot.dx, closeTo(target.dx, 1e-9));
      expect(hotspot.dy, closeTo(target.dy, 1e-9));
      final canvas = _Canvas();
      painter.paint(canvas, _viewport);
      final position = canvas.position! + _hotspot * canvas.factor;
      expect(position.dx, closeTo(target.dx, 1e-9));
      expect(position.dy, closeTo(target.dy, 1e-9));
    });
  }
  for (final style in [kRemoteViewStyleOriginal, kRemoteViewStyleCustom]) {
    for (final (frame, scale, offset) in [
      (_viewport, 2.0, const Offset(-40, -64)),
      (const Size(199, 320), 1.0, const Offset(0, -64)),
      (const Size(198, 320), 1.0, const Offset(1, -64)),
      (const Size(400, 159), 1.0, const Offset(-40, 0)),
      (const Size(400, 158), 1.0, const Offset(-40, 1)),
    ]) {
      testWidgets('$style frame=$frame painted cursor follows scrollbar layout',
          (tester) async {
        final painter = await _paintCursor(
            tester, _ScrollbarCanvasModel(style, frame: frame, scale: scale));
        final target = _remotePosition * scale + offset;
        expect((Offset(painter.x, painter.y) + _hotspot) * painter.scale, target);
      });
    }
  }
}
