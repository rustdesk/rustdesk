import 'dart:math' as math;

const displayAspectRatios = [
  (16, 9),
  (16, 10),
  (4, 3),
  (3, 2),
  (5, 4),
  (21, 9),
  (32, 9),
  (1, 1),
];

(int, int)? displayAspectRatio(int width, int height) {
  if (width <= 0 || height <= 0) return null;
  (int, int)? rounded;
  for (final ratio in displayAspectRatios) {
    final (w, h) = height > width ? (ratio.$2, ratio.$1) : ratio;
    if (width * h == height * w) return (w, h);
    // Pixel rounding also covers modes such as 1366×768 without inventing a
    // separate 683:384 choice for a nominal 16:9 desktop.
    if (rounded == null &&
        ((width * h / w).round() == height ||
            (height * w / h).round() == width)) {
      rounded = (w, h);
    }
  }
  return rounded;
}

int? linkedDisplayDimension(int value, (int, int) ratio,
    {required bool widthChanged}) {
  if (value <= 0 || ratio.$1 <= 0 || ratio.$2 <= 0) return null;
  return widthChanged
      ? (value * ratio.$2 / ratio.$1).round()
      : (value * ratio.$1 / ratio.$2).round();
}

(int, int)? fitDisplayResolution({
  required (int, int) localSize,
  required int minDimension,
  required int maxDimension,
  required int scale,
  required bool allowArbitrarySize,
  required List<(int, int)> supported,
}) {
  final (width, height) = localSize;
  if (width <= 0 || height <= 0 || scale <= 0) return null;
  final minimum = (minDimension / scale).ceil() * scale;
  final maximum = maxDimension ~/ scale * scale;
  if (minimum <= 0 || minimum > maximum) return null;
  if (allowArbitrarySize) {
    final lower = math.max(minimum / width, minimum / height);
    final upper = math.min(maximum / width, maximum / height);
    if (lower > upper) return null;
    final factor = 1.0.clamp(lower, upper);
    int align(int dimension) =>
        ((dimension * factor / scale).round() * scale).clamp(minimum, maximum);
    return (align(width), align(height));
  }

  final candidates = supported
      .where((size) =>
          size.$1 >= minimum &&
          size.$1 <= maximum &&
          size.$2 >= minimum &&
          size.$2 <= maximum &&
          size.$1 % scale == 0 &&
          size.$2 % scale == 0)
      .toSet()
      .toList();
  // Prefer the closest aspect ratio, then the closest pixel count. Never invent
  // or rotate a physical mode that the host did not advertise.
  double aspectError((int, int) size) =>
      math.log((size.$1 / size.$2) / (width / height)).abs();
  double areaError((int, int) size) =>
      math.log((size.$1 * size.$2) / (width * height)).abs();
  candidates.sort((a, b) {
    final aspect = aspectError(a).compareTo(aspectError(b));
    if (aspect != 0) return aspect;
    final area = areaError(a).compareTo(areaError(b));
    if (area != 0) return area;
    final byWidth = a.$1.compareTo(b.$1);
    return byWidth != 0 ? byWidth : a.$2.compareTo(b.$2);
  });
  return candidates.isEmpty ? null : candidates.first;
}
