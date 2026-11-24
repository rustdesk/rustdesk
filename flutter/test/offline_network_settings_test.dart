import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart' show ColorThemeExtension, globalKey;
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _SettingsBinding extends Fake implements RustdeskImpl {
  String unlockMethod = 'offline';
  bool approved = false;
  bool securityApproved = false;

  @override
  bool mainIsInstalled({dynamic hint}) => true;

  @override
  bool mainIsInstalledDaemon({required bool prompt, dynamic hint}) => false;

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<String> mainGetCommon({required String key, dynamic hint}) async {
    if (key == 'unix-unlock-pin:unlock-method') return unlockMethod;
    if (key == 'unix-unlock-pin:admin') return securityApproved.toString();
    throw StateError('Unexpected request: $key');
  }

  @override
  Future<bool> mainCheckSuperUserPermission({dynamic hint}) async => approved;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  if (Platform.isMacOS) {
    setUp(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(const MethodChannel('window_manager'),
              (call) async => call.method == 'isMinimized' ? false : null);
    });
  }

  test('offline Network settings require local administrator approval',
      () async {
    final binding = _SettingsBinding();
    platformFFI.setBindingForTesting(binding);

    expect(await authorizeUnixSettings(networkSettings: true), isFalse);
    binding.approved = true;
    expect(await authorizeUnixSettings(networkSettings: true), isTrue);
  }, skip: !(Platform.isLinux || Platform.isMacOS));

  test('Network approval does not unlock a locked Security page', () async {
    final binding = _SettingsBinding()
      ..unlockMethod = 'admin'
      ..approved = true
      ..securityApproved = false;
    platformFFI.setBindingForTesting(binding);

    expect(await authorizeUnixSettings(networkSettings: true), isTrue);
    expect(await authorizeUnixSettings(), isFalse);
    binding.securityApproved = true;
    expect(await authorizeUnixSettings(), isTrue);
    binding.securityApproved = false;
    expect(await authorizeUnixSettings(), isFalse);
  }, skip: !(Platform.isLinux || Platform.isMacOS));

  testWidgets(
      'offline Security settings stay locked and show the service action',
      (tester) async {
    platformFFI.setBindingForTesting(_SettingsBinding()..approved = true);
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      theme: ThemeData(extensions: [ColorThemeExtension.light]),
      home: const SizedBox(),
    ));

    expect(await authorizeUnixSettings(), isFalse);
    await tester.pump();
    final action = Platform.isMacOS ? 'install_daemon_tip' : 'Start service';
    expect(find.text('Service is not running. $action'), findsOneWidget);
    await tester.pump(const Duration(seconds: 3));
  }, skip: !(Platform.isLinux || Platform.isMacOS));
}
