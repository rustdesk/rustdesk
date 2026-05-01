import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import 'package:flutter_hbb/common.dart';

Future<ui.Image?> decodeImageFromPixels(
  Uint8List pixels,
  int width,
  int height,
  ui.PixelFormat format, {
  int? rowBytes,
  int? targetWidth,
  int? targetHeight,
  bool allowUpscaling = true,
}) async {
  if (targetWidth != null) {
    assert(allowUpscaling || targetWidth <= width);
    if (!(allowUpscaling || targetWidth <= width)) {
      print("not allow upscaling but targetWidth > width");
      return null;
    }
  }
  if (targetHeight != null) {
    assert(allowUpscaling || targetHeight <= height);
    if (!(allowUpscaling || targetHeight <= height)) {
      print("not allow upscaling but targetHeight > height");
      return null;
    }
  }

  final ui.ImmutableBuffer buffer;
  try {
    buffer = await ui.ImmutableBuffer.fromUint8List(pixels);
  } catch (e) {
    return null;
  }

  final ui.ImageDescriptor descriptor;
  try {
    descriptor = ui.ImageDescriptor.raw(
      buffer,
      width: width,
      height: height,
      rowBytes: rowBytes,
      pixelFormat: format,
    );
    if (!allowUpscaling) {
      if (targetWidth != null && targetWidth > descriptor.width) {
        targetWidth = descriptor.width;
      }
      if (targetHeight != null && targetHeight > descriptor.height) {
        targetHeight = descriptor.height;
      }
    }
  } catch (e) {
    print("ImageDescriptor.raw failed: $e");
    buffer.dispose();
    return null;
  }

  final ui.Codec codec;
  try {
    codec = await descriptor.instantiateCodec(
      targetWidth: targetWidth,
      targetHeight: targetHeight,
    );
  } catch (e) {
    print("instantiateCodec failed: $e");
    buffer.dispose();
    descriptor.dispose();
    return null;
  }

  final ui.FrameInfo frameInfo;
  try {
    frameInfo = await codec.getNextFrame();
  } catch (e) {
    print("getNextFrame failed: $e");
    codec.dispose();
    buffer.dispose();
    descriptor.dispose();
    return null;
  }

  codec.dispose();
  buffer.dispose();
  descriptor.dispose();
  return frameInfo.image;
}

// Scale multiplier applied to the remote cursor for better visibility.
const double kCursorScaleFactor = 1.25;

class ImagePainter extends CustomPainter {
  ImagePainter({
    required this.image,
    required this.x,
    required this.y,
    required this.scale,
    this.isCursor = false,
  });

  ui.Image? image;
  double x;
  double y;
  double scale;
  bool isCursor;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    if (x.isNaN || y.isNaN) return;
    final effectiveScale =
        isCursor ? scale * kCursorScaleFactor : scale;
    canvas.scale(effectiveScale, effectiveScale);
    // https://github.com/flutter/flutter/issues/76187#issuecomment-784628161
    // https://api.flutter-io.cn/flutter/dart-ui/FilterQuality.html
    var paint = Paint();
    if ((effectiveScale - 1.0).abs() > 0.001) {
      paint.filterQuality = FilterQuality.medium;
      if (effectiveScale > 10.00000) {
        paint.filterQuality = FilterQuality.high;
      }
    }
    // It's strange that if (scale < 0.5 && paint.filterQuality == FilterQuality.medium)
    // The canvas.drawImage will not work on web
    if (isWeb) {
      paint.filterQuality = FilterQuality.high;
    }
    if (isCursor) {
      // Make cursor white while preserving its alpha shape.
      paint.colorFilter =
          const ColorFilter.mode(Color(0xFFFFFFFF), BlendMode.srcATop);
    }
    final dx = isCursor ? (x / kCursorScaleFactor).toInt().toDouble() : x.toInt().toDouble();
    final dy = isCursor ? (y / kCursorScaleFactor).toInt().toDouble() : y.toInt().toDouble();
    canvas.drawImage(image!, Offset(dx, dy), paint);
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}
