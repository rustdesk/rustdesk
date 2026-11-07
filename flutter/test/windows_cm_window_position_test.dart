import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/windows_cm_window_position.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:window_manager/window_manager.dart';

void main() {
  testWidgets('CM chat stays on its monitor with mixed display scaling',
      (tester) async {
    final messenger = tester.binding.defaultBinaryMessenger;
    const windowChannel = MethodChannel('window_manager');
    const screenChannel = MethodChannel('flutter/windowsize');
    var bounds = const Rect.fromLTWH(3390, 0, 450, 735);
    tester.view.devicePixelRatio = 1.5;
    messenger.setMockMethodCallHandler(screenChannel, (_) async => {
          'frame': [bounds.left, bounds.top, bounds.width, bounds.height],
          'scaleFactor': 1.5,
          'screen': {
            'frame': [1920.0, 0.0, 1920.0, 1080.0],
            'visibleFrame': [1920.0, 0.0, 1920.0, 1008.0],
            'scaleFactor': 1.5,
          },
        });
    messenger.setMockMethodCallHandler(windowChannel, (call) async {
      final args = call.arguments;
      final scale = args['devicePixelRatio'] as double;
      if (call.method == 'setBounds') {
        bounds = Rect.fromLTWH(
          args['x'] == null ? bounds.left : args['x'] * scale,
          args['y'] == null ? bounds.top : args['y'] * scale,
          args['width'] == null ? bounds.width : args['width'] * scale,
          args['height'] == null ? bounds.height : args['height'] * scale,
        );
      } else if (call.method == 'getBounds') {
        return {
          'x': bounds.left / scale,
          'y': bounds.top / scale,
          'width': bounds.width / scale,
          'height': bounds.height / scale,
        };
      }
      return null;
    });
    addTearDown(() {
      messenger.setMockMethodCallHandler(windowChannel, null);
      messenger.setMockMethodCallHandler(screenChannel, null);
      tester.view.resetDevicePixelRatio();
    });

    for (var i = 0; i < 2; i++) {
      await setWindowsCmWindowBounds(const Size(700, 490));
      expect(await windowManager.getBounds(ignoreDevicePixelRatio: true),
          const Rect.fromLTWH(2790, 0, 1050, 735));
      await setWindowsCmWindowBounds(const Size(300, 490));
      expect(await windowManager.getBounds(ignoreDevicePixelRatio: true),
          const Rect.fromLTWH(3390, 0, 450, 735));
    }
  });
}
