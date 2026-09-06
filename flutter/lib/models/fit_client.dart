/// Host framebuffer size advertised in PeerInfo.resolutions.
class HostMode {
  final int width;
  final int height;
  const HostMode(this.width, this.height);

  int get area => width * height;

  @override
  bool operator ==(Object other) =>
      other is HostMode && other.width == width && other.height == height;

  @override
  int get hashCode => Object.hash(width, height);

  @override
  String toString() => '${width}x$height';
}

/// Client viewport used to pick a readable host mode.
class FitClientViewport {
  final double logicalWidth;
  final double logicalHeight;
  final double devicePixelRatio;

  const FitClientViewport({
    required this.logicalWidth,
    required this.logicalHeight,
    required this.devicePixelRatio,
  });

  double get physicalWidth => logicalWidth * devicePixelRatio;
  double get physicalHeight => logicalHeight * devicePixelRatio;
}

/// Uniform adaptive scale that fits [hostW]x[hostH] into the client.
double adaptiveFitScale({
  required double clientW,
  required double clientH,
  required double hostW,
  required double hostH,
}) {
  if (hostW <= 0 || hostH <= 0 || clientW <= 0 || clientH <= 0) {
    return 0;
  }
  final s1 = clientW / hostW;
  final s2 = clientH / hostH;
  return s1 < s2 ? s1 : s2;
}

/// Pick a host mode that still fits on the phone after adaptive scaling.
///
/// A 2880×1800 laptop squeezed onto an iPhone becomes unreadably small.
/// Choosing a smaller advertised mode (for example 1280×800) makes the
/// whole desktop fit *and* leaves icons large enough to tap.
///
/// Portrait phones are treated as landscape: the laptop is not rotated
/// into a 390×844 desktop, which breaks most apps.
HostMode? pickFitClientMode({
  required FitClientViewport viewport,
  required List<HostMode> hostModes,
  int minWidth = 1280,
  int minHeight = 720,
}) {
  if (hostModes.isEmpty) {
    return null;
  }

  var targetW = viewport.physicalWidth;
  var targetH = viewport.physicalHeight;
  if (targetH > targetW) {
    final tmp = targetW;
    targetW = targetH;
    targetH = tmp;
  }

  final unique = <String, HostMode>{};
  for (final mode in hostModes) {
    if (mode.width > 0 && mode.height > 0) {
      unique['${mode.width}x${mode.height}'] = mode;
    }
  }
  final modes = unique.values.toList();
  if (modes.isEmpty) {
    return null;
  }

  bool usableDesktop(HostMode mode) {
    final landscape = mode.width >= mode.height;
    final w = landscape ? mode.width : mode.height;
    final h = landscape ? mode.height : mode.width;
    return w >= minWidth && h >= minHeight;
  }

  var candidates = modes.where(usableDesktop).toList();
  if (candidates.isEmpty) {
    modes.sort((a, b) => b.area.compareTo(a.area));
    return modes.first;
  }

  double score(HostMode mode) {
    final fit = adaptiveFitScale(
      clientW: targetW,
      clientH: targetH,
      hostW: mode.width.toDouble(),
      hostH: mode.height.toDouble(),
    );
    // Higher fit = larger on-phone UI. Cap so a tiny 800×600 does not
    // always win over a still-readable 1280×800 desktop.
    return fit.clamp(0.0, 1.75);
  }

  candidates.sort((a, b) {
    final byScore = score(b).compareTo(score(a));
    if (byScore != 0) {
      return byScore;
    }
    return a.area.compareTo(b.area);
  });
  return candidates.first;
}
