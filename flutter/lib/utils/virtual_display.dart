const kMacOSVirtualDisplayModes = 'macos_virtual_display_modes';

(int, int, int, int)? nativeVirtualDisplayMode(
    Map<String, dynamic> additions, int displayIndex) {
  final modes = additions[kMacOSVirtualDisplayModes];
  if (modes is! Map) return null;
  final mode = modes[displayIndex.toString()];
  if (mode is! List || mode.length != 4 || mode.any((v) => v is! int)) {
    return null;
  }
  final width = mode[0] as int;
  final height = mode[1] as int;
  final scale = mode[2] as int;
  final displayId = mode[3] as int;
  if (displayId <= 0 ||
      displayId > 0xffffffff ||
      width < 320 ||
      width > 4096 ||
      height < 320 ||
      height > 4096 ||
      (scale != 1 && scale != 2) ||
      width % scale != 0 ||
      height % scale != 0) {
    return null;
  }
  return (width, height, scale, displayId);
}

({int width, int height, int minDimension, int maxDimension})
    virtualDisplayResolutionDimensions((int, int, int, int) mode,
        {required bool outputPixels}) {
  final divisor = outputPixels ? 1 : mode.$3;
  return (
    width: mode.$1 ~/ divisor,
    height: mode.$2 ~/ divisor,
    minDimension: (320 / divisor).ceil(),
    maxDimension: 4096 ~/ divisor,
  );
}
