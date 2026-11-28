import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mobile/widgets/app_lock.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final options = <String, String>{};

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      options[key] ?? '';

  @override
  String mainGetAppNameSync({dynamic hint}) => 'RustDesk';
}

void main() {
  late _Binding binding;
  late AppLock lock;
  late FocusNode focusNode;
  var prompts = 0;
  var result = 'ok';

  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    binding.options[kOptionAllowAppLock] = 'Y';
    prompts = 0;
    result = 'ok';
    lock = AppLock(authenticator: (_) async {
      prompts++;
      return result;
    });
    focusNode = FocusNode();
  });

  tearDown(() => focusNode.dispose());

  Future<void> openApp(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      builder: (context, child) =>
          AppLockGate(lock: lock, child: child ?? Container()),
      home: Scaffold(
          body: Focus(focusNode: focusNode, child: const Text('peers'))),
    ));
    await tester.pumpAndSettle();
  }

  Future<void> leaveAndReturn(WidgetTester tester, Duration away) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    lock.onIdle(away.inMilliseconds);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();
  }

  Future<void> tapUnlock(WidgetTester tester) async {
    await tester.tap(find.text('Unlock'));
    await tester.pumpAndSettle();
  }

  testWidgets('the app opens locked until authentication succeeds',
      (tester) async {
    result = 'failed';
    var connected = false;
    lock.whenUnlocked().then((_) => connected = true);
    await openApp(tester);

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();

    expect(prompts, 1);
    expect(find.text('peers').hitTestable(), findsNothing);
    expect(connected, isFalse);

    result = 'ok';
    await tapUnlock(tester);

    expect(prompts, 2);
    expect(find.text('Unlock'), findsNothing);
    expect(find.text('peers').hitTestable(), findsOneWidget);
    expect(connected, isTrue);
  });

  testWidgets('returning within the grace period does not ask again',
      (tester) async {
    await openApp(tester);
    await leaveAndReturn(tester, const Duration(seconds: 59));

    expect(prompts, 1);
    expect(find.text('peers').hitTestable(), findsOneWidget);
  });

  testWidgets('returning after the grace period asks again', (tester) async {
    await openApp(tester);
    result = 'failed';
    await leaveAndReturn(tester, const Duration(seconds: 60));

    expect(prompts, 2);
    expect(find.text('peers').hitTestable(), findsNothing);

    binding.options[kOptionAppLockTimeout] = '0';
    result = 'ok';
    await tapUnlock(tester);
    result = 'failed';
    await leaveAndReturn(tester, Duration.zero);

    expect(prompts, 4);
    expect(find.text('peers').hitTestable(), findsNothing);
  });

  testWidgets('the keyboard cannot reach the app while it is locked',
      (tester) async {
    await openApp(tester);
    focusNode.requestFocus();
    await tester.pump();
    expect(focusNode.hasPrimaryFocus, isTrue);

    result = 'failed';
    await leaveAndReturn(tester, const Duration(minutes: 5));
    expect(focusNode.hasPrimaryFocus, isFalse);

    result = 'ok';
    await tapUnlock(tester);
    expect(focusNode.hasPrimaryFocus, isTrue);
  });

  testWidgets('nothing is locked while the option is off', (tester) async {
    binding.options.remove(kOptionAllowAppLock);
    await openApp(tester);
    await leaveAndReturn(tester, const Duration(minutes: 5));
    await lock.whenUnlocked();

    expect(prompts, 0);
    expect(find.text('peers').hitTestable(), findsOneWidget);
  });
}
