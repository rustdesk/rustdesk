import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:image/image.dart' as img;

int cursorVisibleSize(img.Image image) {
  var left = image.width;
  var top = image.height;
  var right = -1;
  var bottom = -1;
  for (final pixel in image) {
    if (pixel.a == 0) continue;
    if (pixel.x < left) left = pixel.x;
    if (pixel.x > right) right = pixel.x;
    if (pixel.y < top) top = pixel.y;
    if (pixel.y > bottom) bottom = pixel.y;
  }
  if (right < left) return 0;
  final width = right - left + 1;
  final height = bottom - top + 1;
  return width > height ? width : height;
}

class LocalCursorSize extends ValueNotifier<double?> {
  LocalCursorSize() : super(null);

  static const _channel = MethodChannel('org.rustdesk.rustdesk/cursor');
  double? _devicePixelRatio;
  int _generation = 0;

  void ensureLoaded(double devicePixelRatio) {
    if (_devicePixelRatio == devicePixelRatio) return;
    _devicePixelRatio = devicePixelRatio;
    refresh();
  }

  Future<void> refresh() async {
    final generation = ++_generation;
    try {
      final size = await _channel.invokeMethod<double>('getSystemCursorSize');
      if (size == null || !size.isFinite || size <= 0) {
        throw const FormatException('Invalid local system cursor size');
      }
      if (generation == _generation) value = size;
    } catch (error, stack) {
      FlutterError.reportError(FlutterErrorDetails(
          exception: error, stack: stack, library: 'local cursor size'));
    }
  }

  @override
  void dispose() {
    ++_generation;
    super.dispose();
  }
}
