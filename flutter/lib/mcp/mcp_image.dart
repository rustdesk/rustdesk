import 'dart:typed_data';
import 'dart:ui' as ui;

import 'mcp_utils.dart';

Future<List<int>> scaleMcpPng(List<int> png, double scale) async {
  if (scale == 1) return png;
  final size = pngSize(png);
  if (size == null || size.width < 1 || size.height < 1) {
    throw const FormatException('Invalid screenshot dimensions.');
  }
  final width = (size.width * scale).round().clamp(1, size.width);
  final height = (size.height * scale).round().clamp(1, size.height);
  final codec = await ui.instantiateImageCodec(Uint8List.fromList(png),
      targetWidth: width, targetHeight: height);
  try {
    final frame = await codec.getNextFrame();
    try {
      final data = await frame.image.toByteData(format: ui.ImageByteFormat.png);
      if (data == null) throw StateError('Could not encode screenshot.');
      return data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes);
    } finally {
      frame.image.dispose();
    }
  } finally {
    codec.dispose();
  }
}
