import 'package:flutter_hbb/mcp/mcp_dispatcher.dart';
import 'package:flutter_hbb/mcp/mcp_display.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('screenshot pixels map into the selected monitor on mixed-DPI Linux',
      () {
    final session = {
      'displays': [
        {
          'x': -1280,
          'y': 16,
          'width': 1920,
          'height': 1080,
          'input_scale': 1.5
        },
        {'x': 0, 'y': 0, 'width': 1920, 'height': 1080},
      ],
    };
    expect(mcpRemotePoint(session, 0, 960, 540), (x: -640, y: 376));
    expect(mcpRemotePoint(session, 0, 1919, 1079), (x: -1, y: 735));
    expect(mcpRemotePoint(session, 1, 960, 540), (x: 960, y: 540));
    expect(() => mcpRemotePoint(session, 0, 1920, 540),
        throwsA(isA<McpToolException>()));
  });

  Map<String, dynamic> window(List<int> captures, {bool supported = true}) => {
        'displays': [
          for (var i = 0; i < 3; i++) {'index': i}
        ],
        'screenshot_supported': supported,
        'screenshot_displays': captures,
      };

  test('screenshots can use a capture in a sibling window; idle displays fail',
      () {
    final left = window([0]), right = window([2]);
    final connecting = <String, dynamic>{'ready': false};
    mcpShareScreenshotDisplays([left, connecting, right]);
    expect(connecting, {'ready': false});
    expect(left['screenshot_displays'], [0, 2]);
    expect(right['screenshot_displays'], [0, 2]);
    mcpCheckScreenshotDisplay(left, 2);
    expect(
        () => mcpCheckScreenshotDisplay(right, 1),
        throwsA(isA<McpToolException>().having(
            (e) => e.message, 'message', contains('not being captured'))));
  });

  test('peers without the screenshot protocol report an immediate tool error',
      () {
    final old = window([], supported: false);
    expect(
        () => mcpCheckScreenshotDisplay(old, 0),
        throwsA(isA<McpToolException>().having(
            (e) => e.message, 'message', contains('does not support'))));
  });
}
