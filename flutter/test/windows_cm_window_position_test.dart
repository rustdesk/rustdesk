import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/windows_cm_window_position.dart';
import 'package:flutter_test/flutter_test.dart';

/// Toggles the side panel twice and expects each resize to be one `setBounds`
/// on the window's monitor: a resize followed by a move is what let the window
/// cross into a monitor with a different scale.
Future<void> expectSidePanelToggle(
  WidgetTester tester, {
  required double scale,
  required List<double> monitor,
  required List<double> workArea,
  required Rect closed,
  required Rect open,
}) async {
  final messenger = tester.binding.defaultBinaryMessenger;
  const windowChannel = MethodChannel('window_manager');
  const screenChannel = MethodChannel('flutter/windowsize');
  var bounds = closed;
  var setBoundsCalls = 0;
  messenger.setMockMethodCallHandler(
      screenChannel,
      (_) async => {
            'frame': [bounds.left, bounds.top, bounds.width, bounds.height],
            'scaleFactor': scale,
            'screen': {
              'frame': monitor,
              'visibleFrame': workArea,
              'scaleFactor': scale,
            },
          });
  messenger.setMockMethodCallHandler(windowChannel, (call) async {
    if (call.method == 'setBounds') {
      final args = call.arguments;
      final ratio = args['devicePixelRatio'] as double;
      setBoundsCalls++;
      bounds = Rect.fromLTWH(
        args['x'] == null ? bounds.left : args['x'] * ratio,
        args['y'] == null ? bounds.top : args['y'] * ratio,
        args['width'] == null ? bounds.width : args['width'] * ratio,
        args['height'] == null ? bounds.height : args['height'] * ratio,
      );
    }
    return null;
  });
  addTearDown(() {
    messenger.setMockMethodCallHandler(windowChannel, null);
    messenger.setMockMethodCallHandler(screenChannel, null);
  });

  for (var i = 0; i < 2; i++) {
    setBoundsCalls = 0;
    await setWindowsCmWindowBounds(const Size(700, 490));
    expect(setBoundsCalls, 1);
    expect(bounds, open);
    setBoundsCalls = 0;
    await setWindowsCmWindowBounds(const Size(300, 490));
    expect(setBoundsCalls, 1);
    expect(bounds, closed);
  }
}

void main() {
  testWidgets(
      'CM side panel opens in one move at the right edge of a 100% monitor',
      (tester) async {
    await expectSidePanelToggle(
      tester,
      scale: 1.0,
      monitor: [0.0, 0.0, 1920.0, 1080.0],
      workArea: [0.0, 0.0, 1920.0, 1040.0],
      closed: const Rect.fromLTWH(1620, 0, 300, 490),
      open: const Rect.fromLTWH(1220, 0, 700, 490),
    );
  });

  testWidgets('CM side panel keeps its size on a 150% monitor', (tester) async {
    await expectSidePanelToggle(
      tester,
      scale: 1.5,
      monitor: [1920.0, 0.0, 1920.0, 1080.0],
      workArea: [1920.0, 0.0, 1920.0, 1008.0],
      closed: const Rect.fromLTWH(3390, 0, 450, 735),
      open: const Rect.fromLTWH(2790, 0, 1050, 735),
    );
  });
}
