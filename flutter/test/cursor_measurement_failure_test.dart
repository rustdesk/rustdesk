import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
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
  testWidgets('pending or failed size measurement preserves the remote cursor',
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
    final measurement = Completer<double>();
    tester.binding.defaultBinaryMessenger
        .setMockMethodCallHandler(sizeChannel, (_) => measurement.future);
    addTearDown(() => tester.binding.defaultBinaryMessenger
        .setMockMethodCallHandler(sizeChannel, null));
    final cursor = _Cursor(_arrow(2));
    await tester.pumpWidget(_view(cursor));
    expect(registered, isNotEmpty);
    measurement.completeError(PlatformException(code: 'cursor_size'));
    await tester.pump();
    expect(tester.takeException(), isA<PlatformException>());
    expect(cursor.cache.localSize, isNull);
    final scale = Platform.isWindows ? 1.0 : 0.5;
    expect(cursor.cache.scaledWidth, 34 * scale);
    expect(cursor.cache.scaledHeight, 46 * scale);
    expect(cursor.cache.hotx, 8 * scale);
    expect(cursor.cache.hoty, 8 * scale);
    expect(registered.last['hotX'], 8 * scale);
    expect(registered.last['hotY'], 8 * scale);
    await tester.pumpWidget(const SizedBox.shrink());
    cursor.dispose();
  });
}

Widget _view(_Cursor cursor) => MediaQuery(
      data: const MediaQueryData(devicePixelRatio: 2),
      child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => _Image()),
          ChangeNotifierProvider<CanvasModel>(create: (_) => _Canvas(2,
              style: kRemoteViewStyleAdaptive, scale: 0.25)),
          ChangeNotifierProvider<CursorModel>.value(value: cursor),
        ],
        child: ImagePaint(
          ffi: _FFI(),
          id: 'measurement-failure-test',
          zoomCursor: false.obs,
          cursorOverImage: true.obs,
          keyboardEnabled: true.obs,
          remoteCursorMoved: false.obs,
        ),
      ),
    );
