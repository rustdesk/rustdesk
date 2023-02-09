import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

Future<ui.Image> decodeImageFromPixels(
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
  }
  if (targetHeight != null) {
    assert(allowUpscaling || targetHeight <= height);
  }

  final ui.ImmutableBuffer buffer =
      await ui.ImmutableBuffer.fromUint8List(pixels);
  final ui.ImageDescriptor descriptor = ui.ImageDescriptor.raw(
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

  final ui.Codec codec = await descriptor.instantiateCodec(
    targetWidth: targetWidth,
    targetHeight: targetHeight,
  );

  final ui.FrameInfo frameInfo = await codec.getNextFrame();
  codec.dispose();
  buffer.dispose();
  descriptor.dispose();
  return frameInfo.image;
}

class ImagePainter extends CustomPainter {
  ImagePainter({
    required this.image,
    required this.x,
    required this.y,
    required this.scale,
  });

  ui.Image? image;
  double x;
  double y;
  double scale;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    if (x.isNaN || y.isNaN) return;
    canvas.scale(scale, scale);
    // https://github.com/flutter/flutter/issues/76187#issuecomment-784628161
    // https://api.flutter-io.cn/flutter/dart-ui/FilterQuality.html
    var paint = Paint();
    if ((scale - 1.0).abs() > 0.001) {
      paint.filterQuality = FilterQuality.medium;
      if (scale > 10.00000) {
        paint.filterQuality = FilterQuality.high;
      }
    }
    canvas.drawImage(
        image!, Offset(x.toInt().toDouble(), y.toInt().toDouble()), paint);
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}
