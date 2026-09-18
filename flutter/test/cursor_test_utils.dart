import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart' as common;
import 'package:flutter_hbb/models/desktop_render_texture.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;

class CursorTestImage extends ChangeNotifier implements ImageModel {
  CursorTestImage({this.useTextureRender = false});
  @override
  final bool useTextureRender;
  @override
  ui.Image? get image => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class CursorTestCanvas extends ChangeNotifier implements CanvasModel {
  CursorTestCanvas(this.devicePixelRatio,
      {required this.style, required this.scale});

  final String style;
  @override
  final double devicePixelRatio;
  @override
  final imageOverflow = false.obs;
  @override
  late final viewStyle = ViewStyle(
    style: style,
    width: size.width,
    height: size.height,
    displayWidth: 400,
    displayHeight: 320,
  );
  @override
  bool get cursorEmbedded => false;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollauto;
  @override
  Size get size => const Size(200, 160);
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

class CursorTestPeer extends Fake implements FfiModel {
  @override
  final pi = PeerInfo();
  @override
  bool isPeerLinux = false;
}

class CursorTestFFI extends Fake implements FFI {
  CursorTestFFI(this.canvasModel);
  @override
  final CanvasModel canvasModel;
  @override
  final inputModel = _Input();
  @override
  final CursorTestPeer ffiModel = CursorTestPeer();
}

void captureNativeCursors(List<Map<dynamic, dynamic>> registrations,
    {List<String>? activations}) {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final channel = common.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  setUp(() {
    registrations.clear();
    activations?.clear();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      if (call.method.startsWith('setCustomCursor')) {
        activations?.add(call.arguments['name'] as String);
      }
      if (!call.method.startsWith('createCustomCursor')) return null;
      final args = call.arguments as Map<dynamic, dynamic>;
      registrations.add(args);
      return args['name'];
    });
  });
  tearDown(() =>
      binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null));
}

img.Image decodeNativeCursorRaster(Map<dynamic, dynamic> args) {
  final bytes = args['buffer'] as Uint8List;
  return common.isWindows
      ? img.Image.fromBytes(
          width: args['width'] as int,
          height: args['height'] as int,
          bytes: bytes.buffer,
          bytesOffset: bytes.offsetInBytes,
          order: img.ChannelOrder.bgra)
      : img.decodePng(bytes)!;
}

class CursorTestTexture extends Fake implements TextureModel {
  @override
  RxInt getTextureId(int display) => 0.obs;
}

class CursorTestDraw extends Fake implements Canvas {
  double factor = 1;
  Offset? position;
  @override
  void scale(double sx, [double? sy]) => factor *= sx;
  @override
  void drawImage(ui.Image image, Offset offset, Paint paint) =>
      position = offset * factor;
}
