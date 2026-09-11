import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/image.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

const _hotspot = Offset(4, 9);
const _remotePosition = Offset(100.25, 80.75);
const _canvasOffset = Offset(15.125, 10.25);
const _viewport = Size(200, 160);

class _CursorModel extends ChangeNotifier implements CursorModel {
  _CursorModel(this.image);

  @override
  final ui.Image image;
  @override
  double get hotx => _hotspot.dx;
  @override
  double get hoty => _hotspot.dy;
  @override
  double get x => _remotePosition.dx;
  @override
  double get y => _remotePosition.dy;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _CanvasModel extends ChangeNotifier implements CanvasModel {
  _CanvasModel(String style, this.scale)
      : viewStyle = ViewStyle(
          style: style,
          width: _viewport.width,
          height: _viewport.height,
          displayWidth: _viewport.width.toInt(),
          displayHeight: _viewport.height.toInt(),
        );

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
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
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

void main() {
  for (final (style, zoom, dpr, source, canvasScale, scale) in [
    (kRemoteViewStyleAdaptive, false, 2.0, (48, 64), 0.375, 0.375),
    (kRemoteViewStyleAdaptive, false, 3.0, (48, 64), 0.25, 0.25),
    (kRemoteViewStyleAdaptive, true, 3.0, (48, 64), 0.375, 0.375),
    (kRemoteViewStyleOriginal, false, 2.0, (48, 64), 0.5, 0.5),
    (kRemoteViewStyleOriginal, true, 2.0, (48, 64), 0.5, 0.5),
    (
      kRemoteViewStyleAdaptive,
      false,
      2.0,
      (9, 18),
      0.1,
      Platform.isWindows ? 2 / 3 : 4 / 3
    ),
    (
      kRemoteViewStyleAdaptive,
      true,
      2.0,
      (9, 18),
      0.1,
      Platform.isWindows ? 2 / 3 : 4 / 3
    ),
  ]) {
    testWidgets(
        '$style zoom=$zoom dpr=$dpr source=$source keeps remote geometry',
        (tester) async {
      final image = (await tester.runAsync(
          () => createTestImage(width: source.$1, height: source.$2)))!;
      addTearDown(image.dispose);
      await tester.pumpWidget(MediaQuery(
        data: MediaQueryData(devicePixelRatio: dpr),
        child: MultiProvider(
          providers: [
            ChangeNotifierProvider<CursorModel>(
                create: (_) => _CursorModel(image)),
            ChangeNotifierProvider<CanvasModel>(
                create: (_) => _CanvasModel(style, canvasScale)),
          ],
          child: CursorPaint(id: 'cursor-test', zoomCursor: zoom.obs),
        ),
      ));
      final painter = tester
          .widget<CustomPaint>(find.byType(CustomPaint))
          .painter! as ImagePainter;

      expect(painter.image, same(image));
      expect(painter.scale, scale);
      expect((Offset(painter.x, painter.y) + _hotspot) * scale,
          _remotePosition * canvasScale + _canvasOffset);
      final canvas = _Canvas();
      painter.paint(canvas, _viewport);
      final position = canvas.position! + _hotspot * canvas.factor;
      final target = _remotePosition * canvasScale + _canvasOffset;
      expect(position.dx, closeTo(target.dx, 1e-9));
      expect(position.dy, closeTo(target.dy, 1e-9));
    });
  }
}
