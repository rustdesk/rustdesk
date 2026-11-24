import 'dart:ui';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:desktop_multi_window/src/backend.dart';
import 'package:desktop_multi_window/src/window_controller_impl.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const control = MethodChannel('mixin.one/flutter_multi_window');
  const events = MethodChannel('mixin.one/flutter_multi_window_channel');
  final calls = <MethodCall>[];

  setUp(() {
    calls.clear();
    messenger.setMockMethodCallHandler(control, (call) async {
      calls.add(call);
      switch (call.method) {
        case 'createWindow':
          return 7;
        case 'getAllSubWindowIds':
          return [0, 7, 8];
        case 'isMaximized':
          return true;
        default:
          return null;
      }
    });
  });
  tearDown(() {
    experimentalWindowBackend = null;
    DesktopMultiWindow.setMethodHandler(null);
    messenger.setMockMethodCallHandler(control, null);
    messenger.setMockMethodCallHandler(events, null);
  });

  test('default backend keeps legacy channel and controller payloads',
      () async {
    final window = await DesktopMultiWindow.createWindow('boot');
    expect(window.windowId, 7);
    expect(calls.single.method, 'createWindow');
    expect(calls.single.arguments, 'boot');
    await window.show();
    await window.hide();
    await window.close();
    expect(calls.skip(1).map((c) => c.method), ['show', 'hide', 'close']);
    expect(calls.skip(1).every((c) => c.arguments == 7), isTrue);
    await WindowController.fromWindowId(8)
        .setFrame(const Rect.fromLTWH(1, 2, 3, 4));
    expect(calls.last.arguments,
        {'windowId': 8, 'left': 1.0, 'top': 2.0, 'width': 3.0, 'height': 4.0});
    await window.startResizing(SubWindowResizeEdge.topLeft);
    expect((calls.last.arguments as Map)['resizeEdge'], 'topLeft');
    expect(await WindowController.main().isMaximized(), isTrue);
    expect(calls.last.arguments, 0);
    expect(await DesktopMultiWindow.getAllSubWindowIds(), [7, 8]);
  });

  test('legacy cross-window envelope and return value unchanged', () async {
    messenger.setMockMethodCallHandler(events, (call) async {
      expect(call.method, 'hello');
      expect(call.arguments, {
        'targetWindowId': 8,
        'arguments': ['payload']
      });
      return {'reply': 42};
    });
    expect(await DesktopMultiWindow.invokeMethod(8, 'hello', ['payload']),
        {'reply': 42});
  });

  test('legacy inbound handler unwraps sender and arguments', () async {
    DesktopMultiWindow.setMethodHandler((call, from) async {
      expect(from, 8);
      expect(call.method, 'hello');
      expect(call.arguments, 'ping');
      return 'pong';
    });
    Object? reply;
    await messenger.handlePlatformMessage(
        events.name,
        const StandardMethodCodec().encodeMethodCall(const MethodCall('hello', {
          'fromWindowId': 8,
          'arguments': 'ping',
        })),
        (data) => reply = const StandardMethodCodec().decodeEnvelope(data!));
    expect(reply, 'pong');
  });

  test('a selected official window coexists with legacy windows and handlers',
      () async {
    final backend = _HybridBackend();
    experimentalWindowBackend = backend;
    final legacy = await DesktopMultiWindow.createWindow('remote');
    final official = await DesktopMultiWindow.createWindow('port-forward');
    expect(legacy.windowId, 7);
    expect(official.windowId, _HybridBackend.officialId);
    expect(calls.single.arguments, 'remote');
    expect(await DesktopMultiWindow.getAllSubWindowIds(),
        [7, 8, _HybridBackend.officialId]);
    await WindowController.fromWindowId(7).show();
    expect(calls.last.arguments, 7);
    expect(await DesktopMultiWindow.invokeMethod(official.windowId, 'hello'),
        'official: hello');

    DesktopMultiWindow.setMethodHandler((call, from) async => 'main: $from');
    backend.currentWindowId = official.windowId;
    DesktopMultiWindow.setMethodHandler((call, from) async => 'child: $from');
    expect(await backend.handlers[official.windowId]!(const MethodCall('ping'), 0),
        'child: 0');
    Object? reply;
    await messenger.handlePlatformMessage(
      events.name,
      const StandardMethodCodec().encodeMethodCall(const MethodCall('ping', {
        'fromWindowId': 7,
        'arguments': null,
      })),
      (data) => reply = const StandardMethodCodec().decodeEnvelope(data!),
    );
    expect(reply, 'main: 7');
  });
}

class _HybridBackend implements DesktopWindowBackend {
  static const officialId = 1 << 30;
  final handlers = <int, WindowMethodHandler?>{};

  @override
  int currentWindowId = 0;
  @override
  bool get keepsLegacyWindows => true;
  @override
  bool acceptsWindow(String? arguments) => arguments == 'port-forward';
  @override
  bool ownsWindow(int id) => id == 0 || id == officialId;
  @override
  Future<WindowController> createWindow(String? arguments) async =>
      WindowControllerMainImpl(officialId);
  @override
  Future<List<int>> getAllSubWindowIds() async => [officialId];
  @override
  Future<dynamic> invokeMethod(int target, String method, dynamic arguments) async =>
      'official: $method';
  @override
  void setMethodHandler(WindowMethodHandler? handler) =>
      handlers[currentWindowId] = handler;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
