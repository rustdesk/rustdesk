import 'dart:convert';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart' show CursorManager;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/common.dart' as common;
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

const _viewport = Size(200, 160);

class _Image extends ChangeNotifier implements ImageModel {
  @override
  bool get useTextureRender => false;
  @override
  ui.Image? get image => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Canvas extends ChangeNotifier implements CanvasModel {
  _Canvas(this.devicePixelRatio, {required this.style, required this.scale});

  final String style;

  @override
  final double devicePixelRatio;
  @override
  final imageOverflow = false.obs;
  @override
  late final viewStyle = ViewStyle(
    style: style,
    width: _viewport.width,
    height: _viewport.height,
    displayWidth: 400,
    displayHeight: 320,
  );
  @override
  bool get cursorEmbedded => false;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollauto;
  @override
  Size get size => _viewport;
  @override
  final double scale;
  @override
  double get x => 0;
  @override
  double get y => 0;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Input extends Fake implements InputModel {
  @override
  final relativeMouseMode = false.obs;
}

class _Peer extends Fake implements FfiModel {
  @override
  final pi = PeerInfo();
  @override
  bool isPeerLinux = false;
}

class _FFI extends Fake implements FFI {
  _FFI(this.canvasModel);

  @override
  final CanvasModel canvasModel;
  @override
  final inputModel = _Input();
  @override
  final _Peer ffiModel = _Peer();
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  _rasterBoundsTests();
  final channel = common.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  final registrations = <Map<dynamic, dynamic>>[];
  setUp(() {
    registrations.clear();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      if (!call.method.startsWith('createCustomCursor')) return null;
      final args = call.arguments as Map<dynamic, dynamic>;
      registrations.add(args);
      return args['name'];
    });
  });
  tearDown(() =>
      binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null));
  _legacySizingTests(registrations);
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final density in ['0', '1', '2', '1e-300', '0.001']) {
      testWidgets(
          'density boundary $style density=$density',
          (tester) => tester.runAsync(
              () => checkDensity(tester, (style, density), registrations)));
    }
  }
}

void _legacySizingTests(List<Map<dynamic, dynamic>> registrations) {
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final density in [null, '0']) {
      for (final (width, height, dpr) in [
        (4, 32, 1.0),
        (4, 32, 2.0),
        (9, 18, 1.0),
        (9, 18, 2.0),
        (32, 32, 1.0),
        (32, 32, 2.0),
      ]) {
        testWidgets(
            'legacy $style density=$density ${width}x$height DPR=$dpr',
            (tester) => tester.runAsync(() => checkDensity(
                tester, (style, density), registrations,
                sourceSize: Size(width.toDouble(), height.toDouble()),
                dpr: dpr)));
      }
    }
  }
}

void _rasterBoundsTests() {
  for (final (width, height, scale, legacy, rasterScale) in [
    (32, 32, 1e300, false, 1.0),
    (32, 32, 129.0, false, 1.0),
    (32, 32, 100.0, false, 2.0),
    (512, 1, 0.1, true, 1.0),
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
  final canvas = _Canvas(dpr, style: style, scale: 0.5);
  final ffi = _FFI(canvas)..ffiModel.pi.platform = kPeerPlatformMacOS;
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
  final rejected = density == '1e-300' || density == '0.001';
  if (rejected) {
    expect(cursor.cache, isNull,
        reason: 'Reject density before publishing a cursor');
    await cursor.updateCursorData(_cursorEvent(id, '2'));
  }
  expect(cursor.cache!.pixelRatio, rejected ? 2 : double.parse(density ?? '0'));
  await _paintCursor(tester, ffi, cursor);
  expect(tester.takeException(), isNull);
  for (final key in cursor.cachedKeys) {
    await CursorManager.instance.ensureCursorRegistered(key);
  }
  expect(registrations, hasLength(1));
  await tester.pumpWidget(const SizedBox.shrink());
  if (density == null || density == '0') {
    expect(cursor.cache!.scale, 1.0);
    _expectLegacyRaster(registrations.single, sourceSize, dpr);
  }
}

Future<void> _paintCursor(
    WidgetTester tester, _FFI ffi, CursorModel cursor) async {
  await tester.pumpWidget(MediaQuery(
    data: MediaQueryData(devicePixelRatio: ffi.canvasModel.devicePixelRatio),
    child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => _Image()),
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
  final bytes = args['buffer'] as Uint8List;
  final decoded = common.isWindows
      ? img.Image.fromBytes(
          width: width,
          height: height,
          bytes: bytes.buffer,
          bytesOffset: bytes.offsetInBytes,
          order: img.ChannelOrder.bgra)
      : img.decodePng(bytes)!;
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
