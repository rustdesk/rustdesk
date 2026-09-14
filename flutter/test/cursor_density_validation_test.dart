import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart' show CursorManager;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/common.dart' as common;
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

import 'cursor_test_utils.dart';

void main() {
  _rasterBoundsTests();
  final registrations = <Map<dynamic, dynamic>>[];
  captureNativeCursors(registrations);
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final (density, width, dpr) in [
      (null, 4, 1.0),
      ('0', 4, 2.0),
      (null, 32, 2.0),
      ('0', 32, 1.0),
    ]) {
      testWidgets(
          'legacy $style density=$density ${width}x32 DPR=$dpr',
          (tester) => tester.runAsync(() => checkDensity(
              tester, (style, density), registrations,
              sourceSize: Size(width.toDouble(), 32), dpr: dpr)));
    }
  }
}

void _rasterBoundsTests() {
  for (final (width, height, scale, legacy, rasterScale) in [
    (32, 32, 1e300, false, 1.0),
    (32, 32, 100.0, false, 2.0),
  ]) {
    test('rejects raster ${width}x$height scale=$scale before key creation',
        () async {
      final image = await createTestImage(width: width, height: height);
      addTearDown(image.dispose);
      final data = CursorData(
          peerId: 'bounds',
          id: 'bounds',
          image: img.Image(width: width, height: height, numChannels: 4),
          nativeImage: image,
          scale: 1,
          data: null,
          hotxOrigin: 0,
          hotyOrigin: 0,
          width: width,
          height: height);
      final initial = data.updateGetKey(1, resizeImage: false);
      expect(
          data.updateGetKey(scale,
              resizeImage: false,
              useLegacyMinimum: legacy,
              rasterScale: rasterScale),
          isNull);
      expect(data.scale, 1);
      expect(data.updateGetKey(1, resizeImage: false), initial);
    });
  }
}

Future<void> checkDensity(WidgetTester tester, (String, String?) input,
    List<Map<dynamic, dynamic>> registrations,
    {Size sourceSize = const Size(32, 32), double dpr = 1}) async {
  final (style, density) = input;
  final id = '$style-$density';
  tester.view.devicePixelRatio = dpr;
  addTearDown(tester.view.resetDevicePixelRatio);
  final canvas = CursorTestCanvas(dpr, style: style, scale: 0.5);
  final ffi = CursorTestFFI(canvas)..ffiModel.pi.platform = kPeerPlatformMacOS;
  final cursor = CursorModel(WeakReference(ffi))..id = id;
  addTearDown(() async {
    expect(cursor.parent.target, same(ffi));
    for (final key in cursor.cachedKeys) {
      await deleteCustomCursor(key);
    }
    cursor.disposeImages();
    cursor.dispose();
    canvas.dispose();
  });
  await cursor.updateCursorData(_cursorEvent(id, density, size: sourceSize));
  expect(cursor.cache!.pixelRatio, 0);
  await _paintCursor(tester, ffi, cursor);
  expect(tester.takeException(), isNull);
  for (final key in cursor.cachedKeys) {
    await CursorManager.instance.ensureCursorRegistered(key);
  }
  expect(registrations, hasLength(1));
  await tester.pumpWidget(const SizedBox.shrink());
  expect(cursor.cache!.scale, 1.0);
  _expectLegacyRaster(registrations.single, sourceSize, dpr);
}

Future<void> _paintCursor(
    WidgetTester tester, CursorTestFFI ffi, CursorModel cursor) async {
  await tester.pumpWidget(MediaQuery(
    data: MediaQueryData(devicePixelRatio: ffi.canvasModel.devicePixelRatio),
    child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => CursorTestImage()),
          ChangeNotifierProvider<CanvasModel>.value(value: ffi.canvasModel),
          ChangeNotifierProvider<CursorModel>.value(value: cursor),
        ],
        child: ImagePaint(
            ffi: ffi,
            id: 'density-review',
            zoomCursor: false.obs,
            cursorOverImage: true.obs,
            keyboardEnabled: true.obs,
            remoteCursorMoved: false.obs)),
  ));
}

void _expectLegacyRaster(Map<dynamic, dynamic> args, Size size, double dpr) {
  // Base unzoomed sizing is logical on macOS/GTK, physical on Windows.
  final rasterScale = common.isWindows ? 1.0 : dpr;
  final width = (size.width * rasterScale).toInt();
  final height = (size.height * rasterScale).toInt();
  final bufferWidth = common.isLinux ? size.longestSide * rasterScale : width;
  expect((args['width'], args['height']), (bufferWidth, height));
  expect((args['hotX'], args['hotY']),
      ((size.width ~/ 2) * rasterScale, (size.height ~/ 2) * rasterScale));
  expect(args['imagePixelRatio'], dpr);
  final decoded = decodeNativeCursorRaster(args);
  expect((decoded.width, decoded.height), (bufferWidth, height));
  expect(decoded.getPixel(width - 1, height - 1).a, 255);
  if (bufferWidth > width) expect(decoded.getPixel(width, 0).a, 0);
}

Map<String, String> _cursorEvent(String id, String? density,
        {Size size = const Size(32, 32)}) =>
    {
      'id': id,
      'width': '${size.width.toInt()}',
      'height': '${size.height.toInt()}',
      'hotx': '${size.width ~/ 2}',
      'hoty': '${size.height ~/ 2}',
      if (density != null) 'scale': density,
      'colors': jsonEncode(
          List<int>.filled((size.width * size.height * 4).toInt(), 255)),
    };
