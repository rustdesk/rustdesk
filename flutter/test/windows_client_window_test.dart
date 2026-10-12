import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/windows_client_window.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  var visible = false;
  var minimized = false;
  var skipTaskbar = true;
  const channel = MethodChannel('window_manager');
  Client client(bool authorized, {bool fileTransfer = false}) => Client(
      1, authorized, fileTransfer, false, 'Peer', '123', true, true, true);

  setUp(() {
    visible = false;
    minimized = false;
    skipTaskbar = true;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      switch (call.method) {
        case 'isVisible':
          return visible;
        case 'isMinimized':
          return minimized;
        case 'show':
          visible = true;
          break;
        case 'hide':
          visible = false;
          break;
        case 'restore':
          minimized = false;
          break;
        case 'setSkipTaskbar':
          skipTaskbar = call.arguments['isSkipTaskbar'] as bool;
          break;
      }
      return null;
    });
  });

  test('authorized desktop and file transfer connections stay hidden',
      () async {
    await showWindowsApprovalWindow([
      client(true),
      client(true, fileTransfer: true),
    ]);
    expect(visible, isFalse);
    expect(skipTaskbar, isTrue);
  });

  test('connection, voice and elevation approval reveal a minimized window',
      () async {
    final voice = client(true)..incomingVoiceCall = true;
    for (final clients in [
      [client(false)],
      [voice],
      <Client>[]
    ]) {
      visible = false;
      minimized = true;
      skipTaskbar = true;
      await showWindowsApprovalWindow(clients, elevation: clients.isEmpty);
      expect(visible, isTrue);
      expect(minimized, isFalse);
      expect(skipTaskbar, isFalse);
    }
  });

  test('hiding removes the taskbar entry without changing active clients',
      () async {
    final active = client(true);
    visible = true;
    skipTaskbar = false;
    await hideWindowsClientWindow();
    expect(visible, isFalse);
    expect(skipTaskbar, isTrue);
    expect(active.authorized, isTrue);
    expect(active.disconnected, isFalse);
  });
}
