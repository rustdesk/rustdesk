import 'package:flutter/services.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/desktop/widgets/cm_window_dock.dart';
import 'package:flutter_hbb/main.dart' show showCmWindow;
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  String option = '';

  @override
  String mainGetOptionSync({required String key, dynamic hint}) => option;

  @override
  bool mainHideDock({dynamic hint}) => true;
}

void main() {
  test('ordinary CM keeps its original geometry and native window style',
      () async {
    final messenger =
        TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
    final binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    const windowChannel = MethodChannel('window_manager');
    const screenChannel = MethodChannel('screen_retriever');
    const hostChannel = MethodChannel('org.rustdesk.rustdesk/host');
    var bounds = const Rect.fromLTWH(0, 0, 300, 490);
    var minimizable = false;
    var frameless = false;
    var bannerConfigured = false;
    messenger.setMockMethodCallHandler(hostChannel, (call) async {
      if (call.method == 'isCmBannerSupported') return false;
      if (call.method == 'setCmBannerWindow') bannerConfigured = true;
      return null;
    });
    messenger.setMockMethodCallHandler(screenChannel, (call) async {
      const display = {
        'id': 1,
        'size': {'width': 1920.0, 'height': 1080.0},
        'visiblePosition': {'x': 0.0, 'y': 0.0},
      };
      if (call.method == 'getCursorScreenPoint') return {'x': 0.0, 'y': 0.0};
      if (call.method == 'getAllDisplays') return {'displays': [display]};
      return display;
    });
    messenger.setMockMethodCallHandler(windowChannel, (call) async {
      if (call.method == 'isMinimizable') return minimizable;
      if (call.method.startsWith('is')) return false;
      if (call.method == 'setMinimizable') {
        minimizable = call.arguments['isMinimizable'] as bool;
      }
      if (call.method == 'setAsFrameless') frameless = true;
      if (call.method == 'setBounds') {
        final args = call.arguments;
        bounds = Rect.fromLTWH(args['x'] ?? bounds.left, args['y'] ?? bounds.top,
            args['width'] ?? bounds.width, args['height'] ?? bounds.height);
      }
      return null;
    });
    addTearDown(() {
      for (final channel in [windowChannel, screenChannel, hostChannel]) {
        messenger.setMockMethodCallHandler(channel, null);
      }
    });

    await CmWindowDock.initPlatformSupport();
    if (isLinux) expect(CmWindowDock.supported, isFalse);
    for (final option in [if (isLinux) 'Y', '']) {
      binding.option = option;
      final dock = CmWindowDock();
      expect(dock.enabled, isFalse);
      dock.dispose();
      await showCmWindow(isStartup: true);

      expect(bounds, const Rect.fromLTWH(1620, 0, 300, 490));
      expect(minimizable, isFalse);
      expect(frameless, isFalse);
      expect(bannerConfigured, isFalse);
      expect(binding.option, option);
    }
  });
}
