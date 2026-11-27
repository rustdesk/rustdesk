import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart'
    show ColorThemeExtension, gFFI, globalKey, initGlobalFFI, isTest;
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/consts.dart' show kOptionDisableUnlockPin;
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _SettingsBinding extends Fake implements RustdeskImpl {
  String unlockMethod = 'offline';
  bool approved = false;
  bool securityApproved = false;
  bool pinDisabled = false;
  String storedPin = '4321';

  @override
  String mainGetBuildinOption({required String key, dynamic hint}) =>
      key == kOptionDisableUnlockPin && pinDisabled ? 'Y' : '';

  @override
  String mainGetLocalOption({required String key, dynamic hint}) => '';
  @override
  String getLocalFlutterOption({required String k, dynamic hint}) => '';
  @override
  bool isDisableAb({dynamic hint}) => true;
  @override
  bool isDisableGroupPanel({dynamic hint}) => true;

  @override
  int mainMaxEncryptLen({dynamic hint}) => 128;

  @override
  String mainSetUnlockPin({required String pin, dynamic hint}) {
    storedPin = pin;
    return '';
  }

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
    binding.pinDisabled = true;
    expect(await authorizeUnixSettings(), isFalse);
  }, skip: !(Platform.isLinux || Platform.isMacOS));

  testWidgets(
      'offline Security settings require the service unless the PIN is disabled',
      (tester) async {
    final binding = _SettingsBinding()..approved = true;
    platformFFI.setBindingForTesting(binding);
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

    binding.pinDisabled = true;
    expect(await authorizeUnixSettings(), isTrue);
    binding.approved = false;
    expect(await authorizeUnixSettings(), isFalse);
  }, skip: !(Platform.isLinux || Platform.isMacOS));

  Future<void> pumpPinDialog(
      WidgetTester tester, _SettingsBinding binding) async {
    platformFFI.setBindingForTesting(binding);
    isTest = true;
    await initGlobalFFI();
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      theme: ThemeData(extensions: [ColorThemeExtension.light]),
      home: const Scaffold(),
    ));
    addTearDown(gFFI.dialogManager.dismissAll);
  }

  testWidgets('only Remove clears an installed Unix PIN', (tester) async {
    final binding = _SettingsBinding();
    await pumpPinDialog(tester, binding);
    changeUnlockPinDialog(null, (_) {});
    await tester.pumpAndSettle();
    await tester.tap(find.text('OK'));
    await tester.pumpAndSettle();
    expect(binding.storedPin, '4321');

    changeUnlockPinDialog(null, (_) {}, hasHiddenPin: true);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove'));
    await tester.pumpAndSettle();
    expect(binding.storedPin, isEmpty);
  });

  testWidgets('cancel ignores a pending PIN verification', (tester) async {
    await pumpPinDialog(tester, _SettingsBinding());
    final verification = Completer<bool>();
    final accepted = checkUnlockPinDialog((_) => verification.future);
    await tester.pumpAndSettle();
    await tester.tap(find.text('OK'));
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(await accepted, isFalse);

    verification.complete(false);
    await tester.pump();
    expect(tester.takeException(), isNull);
  });
}
