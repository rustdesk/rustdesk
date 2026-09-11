import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/cursor_size.dart';
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
  _Canvas(this.devicePixelRatio);

  @override
  final double devicePixelRatio;
  @override
  final imageOverflow = false.obs;
  @override
  final viewStyle = ViewStyle(
    style: kRemoteViewStyleAdaptive,
    width: _viewport.width,
    height: _viewport.height,
    displayWidth: 400,
    displayHeight: 320,
  );
  @override
  bool get cursorEmbedded => false;
  @override
  Size get size => _viewport;
  @override
  double get scale => 0.5;
  @override
  double get x => 0;
  @override
  double get y => 0;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Cursor extends ChangeNotifier implements CursorModel {
  _Cursor(this.cache);

  @override
  CursorData cache;
  @override
  ui.Image? get image => null;
  @override
  double get hotx => cache.hotxOrigin;
  @override
  double get hoty => cache.hotyOrigin;
  @override
  final Set<String> cachedKeys = {};
  @override
  void addKey(String key) => cachedKeys.add(key);
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
  @override
  final inputModel = _Input();
  @override
  final ffiModel = _Peer();
}

CursorData _cursor(img.Image image,
    {String id = 'sprite', double hotspot = 0}) {
  return CursorData(
    peerId: 'local-size-test',
    id: id,
    image: image,
    scale: 1,
    data: Platform.isWindows
        ? image.getBytes(order: img.ChannelOrder.bgra)
        : Uint8List.fromList(img.encodePng(image)),
    hotxOrigin: hotspot,
    hotyOrigin: hotspot,
    width: image.width,
    height: image.height,
  );
}

CursorData _arrow(int density) {
  final image =
      img.Image(width: 17 * density, height: 23 * density, numChannels: 4);
  img.fill(image, color: img.ColorRgba8(255, 255, 255, 255));
  return _cursor(image, id: 'arrow-$density', hotspot: 4.0 * density);
}

void main() {
  test('local size preserves remote pixels, padding and asymmetric hotspot',
      () {
    final artwork = img.Image(width: 40, height: 48, numChannels: 4);
    img.fillRect(artwork,
        x1: 8,
        y1: 12,
        x2: 22,
        y2: 35,
        color: img.ColorRgba8(180, 90, 45, 128),
        alphaBlend: false);
    for (final density in [1, 2, 3]) {
      final source = img.copyResize(artwork,
          width: artwork.width * density,
          height: artwork.height * density,
          interpolation: img.Interpolation.nearest);
      expect(cursorVisibleSize(source), 24 * density);
      final cursor = CursorData(
        peerId: 'local-size-test',
        id: 'padded-$density',
        image: source,
        scale: 1,
        data: Platform.isWindows
            ? source.getBytes(order: img.ChannelOrder.bgra)
            : Uint8List.fromList(img.encodePng(source)),
        hotxOrigin: 10.0 * density,
        hotyOrigin: 17.0 * density,
        width: source.width,
        height: source.height,
      )..localSize = 24;
      cursor.updateGetKey(0.01);
      final output = Platform.isWindows
          ? img.Image.fromBytes(
              width: cursor.scaledWidth,
              height: cursor.scaledHeight,
              bytes: cursor.data!.buffer,
              order: img.ChannelOrder.bgra)
          : img.decodePng(cursor.data!)!;
      expect(cursorVisibleSize(output), 24);
      expect(output.getBytes(), artwork.getBytes());
      expect((cursor.hotx, cursor.hoty), (10, 17));
    }
  });

  test('local cursor minimum keeps a thin remote cursor visible', () {
    final thin = img.Image(width: 1, height: 48, numChannels: 4);
    img.fill(thin, color: img.ColorRgba8(255, 255, 255, 255));
    final cursor = _cursor(thin)..localSize = 2;
    cursor.updateGetKey(1);
    expect(cursor.scaledWidth, 1);
    expect(cursor.scaledHeight, kMinCursorSize);
    final blank = _cursor(img.Image(width: 17, height: 23, numChannels: 4))
      ..localSize = 24;
    blank.updateGetKey(1);
    final output = Platform.isWindows
        ? img.Image.fromBytes(
            width: 17,
            height: 23,
            bytes: blank.data!.buffer,
            order: img.ChannelOrder.bgra)
        : img.decodePng(blank.data!)!;
    expect(cursorVisibleSize(output), 0);
    expect(blank.scale, 1);
  });

  test('switching local sizing keeps bitmap data and cache geometry consistent',
      () {
    final pending = _arrow(1)..data = null;
    pending.updateGetKey(1);
    expect(pending.data, isNull);
    final cursor = _arrow(1);
    const scale = 1.5;
    final originalKey = cursor.updateGetKey(scale);
    cursor.localSize = 23 * scale;
    final localKey = cursor.updateGetKey(scale);
    expect(cursor.scaledWidth, 26);
    if (Platform.isWindows) {
      expect(localKey, isNot(originalKey));
      expect(cursor.data!.length, 26 * 35 * 4);
    }
    cursor.localSize = null;
    expect(cursor.updateGetKey(scale), originalKey);
    if (Platform.isWindows) expect(cursor.data!.length, 25 * 34 * 4);
  });

  for (final dpr in [1.0, 2.0, 3.0]) {
    testWidgets('Zoom off at local DPR $dpr ignores remote raster density',
        (tester) async {
      final channel = Platform.isWindows
          ? SystemChannels.mouseCursor
          : const MethodChannel('flutter_custom_cursor');
      final registered = <Map<dynamic, dynamic>>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
          (call) async {
        if (call.method.startsWith('createCustomCursor')) {
          final arguments = call.arguments as Map<dynamic, dynamic>;
          registered.add(arguments);
          return arguments['name'];
        }
        return null;
      });
      addTearDown(() => tester.binding.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null));
      const sizeChannel = MethodChannel('org.rustdesk.rustdesk/cursor');
      tester.binding.defaultBinaryMessenger
          .setMockMethodCallHandler(sizeChannel, (call) async {
        expect(call.method, 'getSystemCursorSize');
        return Platform.isWindows ? 23.0 * dpr : 23.0;
      });
      addTearDown(() => tester.binding.defaultBinaryMessenger
          .setMockMethodCallHandler(sizeChannel, null));
      final cursor = _Cursor(_arrow(1));
      await tester.pumpWidget(MediaQuery(
        data: MediaQueryData(devicePixelRatio: dpr),
        child: MultiProvider(
          providers: [
            ChangeNotifierProvider<ImageModel>(create: (_) => _Image()),
            ChangeNotifierProvider<CanvasModel>(create: (_) => _Canvas(dpr)),
            ChangeNotifierProvider<CursorModel>.value(value: cursor),
          ],
          child: ImagePaint(
            ffi: _FFI(),
            id: 'local-size-test',
            zoomCursor: false.obs,
            cursorOverImage: true.obs,
            keyboardEnabled: true.obs,
            remoteCursorMoved: false.obs,
          ),
        ),
      ));
      await tester.pump();
      expect(registered, hasLength(1));
      final original = registered.single;
      cursor.cache = _arrow(2);
      cursor.notifyListeners();
      await tester.pump();
      expect(registered, hasLength(2));
      final retina = registered.last;
      expect(
        [retina['width'], retina['height'], retina['hotX'], retina['hotY']],
        [
          original['width'],
          original['height'],
          original['hotX'],
          original['hotY']
        ],
        reason: 'Only the remote raster density changed; the local cursor must '
            'retain its size and hotspot.',
      );
      await tester.pumpWidget(const SizedBox.shrink());
      cursor.dispose();
    });
  }
}
