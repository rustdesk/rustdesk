import 'dart:math' as math;

import 'mcp_dispatcher.dart';

Map<String, dynamic> _display(Map<String, dynamic> session, int index) {
  final displays = (session['displays'] as List?) ?? const [];
  if (index < 0 || index >= displays.length) {
    throw McpToolException(
        'Display $index does not exist; this session has ${displays.length}.');
  }
  return displays[index] as Map<String, dynamic>;
}

/// Each monitor window can request a separate set of captures on the same
/// native connection. Only report indices that still exist in this window.
void mcpShareScreenshotDisplays(List<Map<String, dynamic>> windows) {
  if (windows.length < 2) return;
  final captured = <int>{
    for (final window in windows)
      if (window['screenshot_supported'] == true)
        ...(window['screenshot_displays'] as List).cast<int>(),
  };
  for (final window in windows) {
    if (window['screenshot_supported'] != true) continue;
    final count = (window['displays'] as List).length;
    window['screenshot_displays'] = [
      for (var i = 0; i < count; i++)
        if (captured.contains(i)) i,
    ];
  }
}

void mcpCheckScreenshotDisplay(Map<String, dynamic> session, int display) {
  _display(session, display);
  if (session['screenshot_supported'] != true) {
    throw McpToolException('This peer does not support screenshot requests.');
  }
  if (!(session['screenshot_displays'] as List).contains(display)) {
    throw McpToolException('Display $display is not being captured. Select it '
        'in a remote window or choose a display from screenshot_displays.');
  }
}

({int x, int y}) mcpRemotePoint(
    Map<String, dynamic> session, int display, int x, int y) {
  final d = _display(session, display);
  final width = d['width'] as int, height = d['height'] as int;
  if (x < 0 || y < 0 || x >= width || y >= height) {
    throw McpToolException(
        'Point ($x, $y) is outside display $display (${width}x$height).');
  }
  final scale = d['input_scale'] as num? ?? 1;
  if (!scale.isFinite || scale < 1) {
    throw McpToolException('Display $display has an invalid input scale.');
  }
  // The remote canvas truncates the scaled display extent and clamps the
  // pointer to its last logical pixel before sending integer coordinates.
  int coordinate(int origin, int pixel, int length) {
    final extent = math.max(1, length ~/ scale);
    return (origin + pixel / scale).toInt().clamp(origin, origin + extent - 1);
  }
  return (
    x: coordinate(d['x'] as int, x, width),
    y: coordinate(d['y'] as int, y, height),
  );
}
