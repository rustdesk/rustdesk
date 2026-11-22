/// Peer flutter-option prefix. One key per remote display index.
const String kRemoteDisplayScreenKeyPrefix = 'remote_display_screen_';

/// Screen origins from another DPI or a 1px rounding error still match.
const double kRemoteDisplayScreenOriginTolerance = 8;

class DisplayRect {
  final double left;
  final double top;
  final double right;
  final double bottom;

  const DisplayRect(this.left, this.top, this.right, this.bottom);

  double get width => right - left;
  double get height => bottom - top;

  @override
  bool operator ==(Object other) =>
      other is DisplayRect &&
      other.left == left &&
      other.top == top &&
      other.right == right &&
      other.bottom == bottom;

  @override
  int get hashCode => Object.hash(left, top, right, bottom);
}

class ScreenOrigin {
  final double x;
  final double y;

  const ScreenOrigin(this.x, this.y);

  @override
  bool operator ==(Object other) =>
      other is ScreenOrigin && other.x == x && other.y == y;

  @override
  int get hashCode => Object.hash(x, y);
}

String remoteDisplayScreenKey(int display) =>
    '$kRemoteDisplayScreenKeyPrefix$display';

String encodeScreenOrigin(double left, double top) => '$left,$top';

ScreenOrigin? decodeScreenOrigin(String raw) {
  final parts = raw.split(',');
  if (parts.length != 2) return null;
  final x = double.tryParse(parts[0].trim());
  final y = double.tryParse(parts[1].trim());
  if (x == null || y == null) return null;
  return ScreenOrigin(x, y);
}

/// Remote display shown by one window. Before peer info arrives, the window's
/// requested display is the only value that is not the primary display.
int? shownRemoteDisplay({
  required bool peerReady,
  required int currentDisplay,
  required int? initialDisplay,
}) {
  if (!peerReady && initialDisplay != null && initialDisplay >= 0) {
    return initialDisplay;
  }
  if (currentDisplay >= 0) return currentDisplay;
  if (initialDisplay != null && initialDisplay >= 0) return initialDisplay;
  return null;
}

String? remoteDisplayEntry({required String peerId, required int? display}) {
  if (peerId.isEmpty || display == null || display < 0) return null;
  return '$peerId:$display';
}

({String peerId, int display})? parseRemoteDisplayEntry(String raw) {
  final sep = raw.lastIndexOf(':');
  if (sep <= 0 || sep >= raw.length - 1) return null;
  final display = int.tryParse(raw.substring(sep + 1));
  if (display == null || display < 0) return null;
  final peerId = raw.substring(0, sep);
  if (peerId.isEmpty) return null;
  return (peerId: peerId, display: display);
}

/// Screen that owns most of [window]. A fullscreen window matches its monitor
/// even when the frame is a pixel outside the reported bounds.
DisplayRect? screenForWindow(List<DisplayRect> screens, DisplayRect window) {
  DisplayRect? best;
  var bestArea = 0.0;
  for (final screen in screens) {
    final overlapLeft = screen.left > window.left ? screen.left : window.left;
    final overlapTop = screen.top > window.top ? screen.top : window.top;
    final overlapRight =
        screen.right < window.right ? screen.right : window.right;
    final overlapBottom =
        screen.bottom < window.bottom ? screen.bottom : window.bottom;
    final width = overlapRight - overlapLeft;
    final height = overlapBottom - overlapTop;
    if (width <= 0 || height <= 0) continue;
    final area = width * height;
    if (area > bestArea) {
      best = screen;
      bestArea = area;
    }
  }
  return best;
}

/// Local screen for each remote display, index-aligned.
///
/// With no saved origin, display `i` stays on local screen `i`. A saved origin
/// claims that screen. Two displays that claim one screen, or a saved origin
/// that is no longer present, take the next unused screen in enumeration order.
List<DisplayRect?> assignRemoteDisplaysToLocalScreens({
  required int remoteCount,
  required List<DisplayRect> localScreens,
  required Map<int, ScreenOrigin> savedOrigins,
}) {
  if (remoteCount <= 0 || localScreens.isEmpty) return [];
  final n =
      remoteCount < localScreens.length ? remoteCount : localScreens.length;
  final hasSaved = [
    for (var i = 0; i < n; i++) i,
  ].any(savedOrigins.containsKey);
  if (!hasSaved) {
    return [for (var i = 0; i < n; i++) localScreens[i]];
  }

  final used = List<bool>.filled(localScreens.length, false);
  final assigned = List<DisplayRect?>.filled(n, null);
  for (var i = 0; i < n; i++) {
    final origin = savedOrigins[i];
    if (origin == null) continue;
    for (var s = 0; s < localScreens.length; s++) {
      if (used[s]) continue;
      final screen = localScreens[s];
      if ((screen.left - origin.x).abs() <=
              kRemoteDisplayScreenOriginTolerance &&
          (screen.top - origin.y).abs() <=
              kRemoteDisplayScreenOriginTolerance) {
        assigned[i] = screen;
        used[s] = true;
        break;
      }
    }
  }
  var next = 0;
  for (var i = 0; i < n; i++) {
    if (assigned[i] != null) continue;
    while (next < localScreens.length && used[next]) {
      next++;
    }
    if (next >= localScreens.length) break;
    assigned[i] = localScreens[next];
    used[next] = true;
  }
  return assigned;
}
