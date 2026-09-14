import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

class _Canvas extends ChangeNotifier implements CanvasModel {
  _Canvas(String style)
      : viewStyle = ViewStyle(
            style: style,
            width: 200,
            height: 160,
            displayWidth: 400,
            displayHeight: 320);
  @override
  final ViewStyle viewStyle;
  @override
  final devicePixelRatio = 1.0;
  @override
  final scale = 0.25;
  @override
  final imageOverflow = false.obs;
  @override
  bool get cursorEmbedded => false;
  @override
  Size get size => const Size(200, 160);
  @override
  double get x => 0;
  @override
  double get y => 0;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Image extends ChangeNotifier implements ImageModel {
  @override
  bool get useTextureRender => false;
  @override
  ui.Image? get image => null;
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
  bool get isPeerLinux => false;
}

class _FFI extends Fake implements FFI {
  _FFI(this.canvasModel);
  @override
  final CanvasModel canvasModel;
  @override
  final ffiModel = _Peer();
  @override
  final inputModel = _Input();
}

class _Cursor extends CursorModel {
  _Cursor(this.cache, FFI ffi) : super(WeakReference(ffi));
  @override
  final CursorData cache;
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final channel = Platform.isWindows
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
  for (final scenario in [
    ((4, 64), 0.5, (2, 32)),
    ((64, 4), 0.5, (32, 2)),
    ((1, 64), 0.25, (1, 16)),
    ((8, 8), 0.5, (12, 12)),
    ((4, 64), 1.0, (4, 64)),
  ]) {
    test('native cursor size $scenario',
        () => _checkSize(scenario, registrations));
  }
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final zoom in [false, true]) {
      testWidgets('$style zoom=$zoom follows the live DPR',
          (tester) => _checkView(tester, (style, zoom), registrations));
    }
  }
}

CursorData _data((int, int) size) {
  final image = img.Image(width: size.$1, height: size.$2, numChannels: 4);
  for (final pixel in image) {
    pixel.setRgba(64, 32, 16, 255);
  }
  return CursorData(
      peerId: 'size',
      id: '$size',
      image: image,
      scale: 1,
      data: Platform.isWindows
          ? image.getBytes(order: img.ChannelOrder.bgra)
          : Uint8List.fromList(img.encodePng(image)),
      hotxOrigin: 0,
      hotyOrigin: 0,
      width: size.$1,
      height: size.$2);
}

Future<void> _dispose(_Cursor cursor) async {
  for (final key in cursor.cachedKeys) {
    await deleteCustomCursor(key);
  }
  cursor.dispose();
}

Future<void> _checkSize(((int, int), double, (int, int)) scenario,
    List<Map<dynamic, dynamic>> registrations) async {
  final ffi = _FFI(_Canvas(kRemoteViewStyleAdaptive));
  final cursor = _Cursor(_data(scenario.$1), ffi);
  addTearDown(() => _dispose(cursor));
  addTearDown(ffi.canvasModel.dispose);
  buildCursorOfCache(cursor, scenario.$2, cursor.cache);
  await Future<void>.delayed(Duration.zero);
  _expectSize(registrations.single, scenario.$3);
}

void _expectSize(Map<dynamic, dynamic> args, (int, int) expected) {
  expect((args['width'], args['height']), expected);
  final bytes = args['buffer'] as Uint8List;
  if (Platform.isWindows) {
    const channels = 4;
    expect(bytes.length, expected.$1 * expected.$2 * channels);
  } else {
    final bitmap = img.decodePng(bytes)!;
    expect((bitmap.width, bitmap.height), expected);
  }
}

Future<void> _checkView(WidgetTester tester, (String, bool) mode,
    List<Map<dynamic, dynamic>> registrations) async {
  const sourceSize = 64;
  final canvas = _Canvas(mode.$1);
  final ffi = _FFI(canvas);
  final cursor = _Cursor(_data((sourceSize, sourceSize)), ffi);
  addTearDown(() => _dispose(cursor));
  addTearDown(canvas.dispose);
  addTearDown(tester.view.resetDevicePixelRatio);
  for (final dpr in [1.0, 1.25, 2.0]) {
    tester.view.devicePixelRatio = dpr;
    await tester.pumpWidget(MediaQuery(
      data: MediaQueryData(devicePixelRatio: dpr),
      child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => _Image()),
          ChangeNotifierProvider<CanvasModel>.value(value: canvas),
          ChangeNotifierProvider<CursorModel>.value(value: cursor),
        ],
        child: ImagePaint(
          ffi: ffi,
          id: 'size',
          zoomCursor: mode.$2.obs,
          cursorOverImage: true.obs,
          keyboardEnabled: true.obs,
          remoteCursorMoved: false.obs,
        ),
      ),
    ));
    final scale = mode.$2
        ? (Platform.isWindows ? canvas.scale * dpr : canvas.scale)
        : (Platform.isWindows ? 1.0 : 1.0 / dpr);
    final expected = (sourceSize * scale).ceil();
    _expectSize(registrations.last, (expected, expected));
  }
  await tester.pumpWidget(const SizedBox.shrink());
}
