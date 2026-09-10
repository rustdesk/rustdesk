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
const _remotePosition = Offset(100, 80);
const _canvasOffset = Offset(15, 10);
const _canvasScale = 0.5;
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
  _CanvasModel(String style)
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
  double get scale => _canvasScale;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollauto;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  for (final (style, zoom, dpr, scale) in [
    (kRemoteViewStyleAdaptive, false, 2.0, 0.5),
    (kRemoteViewStyleAdaptive, false, 3.0, 1 / 3),
    (kRemoteViewStyleAdaptive, true, 3.0, _canvasScale),
    (kRemoteViewStyleOriginal, false, 2.0, _canvasScale),
  ]) {
    testWidgets('$style zoom=$zoom dpr=$dpr keeps cursor scale and hotspot',
        (tester) async {
      final image =
          (await tester.runAsync(() => createTestImage(width: 9, height: 18)))!;
      addTearDown(image.dispose);
      await tester.pumpWidget(MediaQuery(
        data: MediaQueryData(devicePixelRatio: dpr),
        child: MultiProvider(
          providers: [
            ChangeNotifierProvider<CursorModel>(
                create: (_) => _CursorModel(image)),
            ChangeNotifierProvider<CanvasModel>(
                create: (_) => _CanvasModel(style)),
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
          _remotePosition * _canvasScale + _canvasOffset);
    }, skip: Platform.isWindows && style == kRemoteViewStyleAdaptive && !zoom);
  }
}
