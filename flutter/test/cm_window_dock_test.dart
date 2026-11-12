import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/server_page.dart';
import 'package:flutter_hbb/desktop/widgets/cm_window_dock.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/main.dart' show showCmWindow;
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_test/flutter_test.dart' hide isWindows, isLinux;
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

class _Binding extends Fake implements RustdeskImpl {
  String option = '';

  @override
  String mainGetOptionSync({required String key, dynamic hint}) => option;

  @override
  bool mainHideDock({dynamic hint}) => true;

  @override
  String mainGetLocalOption({required String key, dynamic hint}) => '';

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) => '';

  @override
  bool isDisableAb({dynamic hint}) => true;

  @override
  bool isDisableGroupPanel({dynamic hint}) => true;

  @override
  bool cmCanElevate({dynamic hint}) => true;

  @override
  String mainGetBuildinOption({required String key, dynamic hint}) => '';

  @override
  String mainSupportedPrivacyModeImpls({dynamic hint}) => '["test"]';

  @override
  bool isIncomingOnly({dynamic hint}) => false;

  @override
  bool isOutgoingOnly({dynamic hint}) => false;

  @override
  String mainGetAppNameSync({dynamic hint}) => 'RustDesk';

  @override
  String mainGetPeerOptionSync(
          {required String id, required String key, dynamic hint}) =>
      '';

  @override
  String mainGetPeerSync({required String id, dynamic hint}) => '{"info": {}}';

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;
}

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  final binding = _Binding();

  setUp(() {
    binding.option = '';
    platformFFI.setBindingForTesting(binding);
    messenger.setMockMethodCallHandler(
        const MethodChannel('org.rustdesk.rustdesk/host'), (call) async => null);
  });

  tearDown(() => messenger.setMockMethodCallHandler(
      const MethodChannel('org.rustdesk.rustdesk/host'), null));

  test('CM docking is opt-in with Y', () async {
    for (final option in ['', 'N', 'true']) {
      binding.option = option;
      final dock = CmWindowDock();
      addTearDown(dock.dispose);
      expect(dock.enabled, isFalse);
      await dock.dock();
      expect(dock.value, isFalse);
    }
    binding.option = 'Y';
    final dock = CmWindowDock();
    addTearDown(dock.dispose);
    expect(dock.enabled, isTrue);
    expect(option2bool(kOptionAllowAcceptWindowBanner, ''), isFalse);
    expect(option2bool(kOptionAllowAcceptWindowBanner, 'Y'), isTrue);
  });

  test('option changes apply only to the next CM instance', () {
    for (final option in ['', 'Y']) {
      binding.option = option;
      final current = CmWindowDock();
      addTearDown(current.dispose);
      binding.option = option == 'Y' ? '' : 'Y';
      final next = CmWindowDock();
      addTearDown(next.dispose);
      expect(current.enabled, option == 'Y');
      expect(next.enabled, option != 'Y');
    }
  });

  test('native minimization follows the docking option', () async {
    const channel = MethodChannel('window_manager');
    var minimizable = true;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'isMinimizable') return minimizable;
      if (call.method == 'setMinimizable') {
        minimizable = call.arguments['isMinimizable'] as bool;
        return null;
      }
      if (call.method == 'setTitleBarStyle' || call.method == 'setAsFrameless') {
        return null;
      }
      throw UnsupportedError(call.method);
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    for (final option in ['Y', '']) {
      binding.option = option;
      final dock = CmWindowDock();
      addTearDown(dock.dispose);
      await dock.syncMinimizable();
      expect(await windowManager.isMinimizable(), option != 'Y');
    }
  });

  test(
      'opening chat or file transfers only widens compact CM',
      () async {
    const channel = MethodChannel('window_manager');
    var bounds = const Rect.fromLTWH(1884, 864, 36, 72);
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'setAsFrameless') return null;
      if (call.method == 'getBounds') {
        return {
          'x': bounds.left,
          'y': bounds.top,
          'width': bounds.width,
          'height': bounds.height
        };
      }
      if (call.method == 'setBounds') {
        final args = call.arguments;
        bounds =
            Rect.fromLTWH(args['x'], args['y'], args['width'], args['height']);
        return null;
      }
      if (call.method == 'setTitleBarStyle') return null;
      throw UnsupportedError(call.method);
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final dock = CmWindowDock()..value = true;
    addTearDown(dock.dispose);
    await dock.resizeExpanded(kConnectionManagerWindowSizeClosedChat);
    expect(await windowManager.getBounds(),
        const Rect.fromLTWH(1584, 720, 336, 280));
    dock.value = false;
    await dock.resizeExpanded(kConnectionManagerWindowSizeOpenChat);
    expect(await windowManager.getBounds(),
        const Rect.fromLTWH(1184, 720, 736, 280));
    await dock.resizeExpanded(kConnectionManagerWindowSizeClosedChat);
    expect(await windowManager.getBounds(),
        const Rect.fromLTWH(1584, 720, 336, 280));
  });

  test('CM first appears compact at the bottom right', () async {
    binding.option = 'Y';
    final oldIsTest = isTest;
    final oldDesktopType = desktopType;
    isTest = true;
    desktopType = DesktopType.cm;
    await initGlobalFFI();
    addTearDown(() {
      isTest = oldIsTest;
      desktopType = oldDesktopType;
    });
    const channel = MethodChannel('window_manager');
    const screenChannel = MethodChannel('screen_retriever');
    const display = {
      'id': 1,
      'size': {'width': 1920.0, 'height': 1080.0},
      'visiblePosition': {'x': 0.0, 'y': 0.0},
    };
    var bounds = const Rect.fromLTWH(1620, 0, 300, 490);
    final visibleBounds = <Rect>[];
    final visibleTaskbarEntries = <bool>[];
    var skipTaskbar = false;
    messenger.setMockMethodCallHandler(screenChannel, (call) async {
      if (call.method == 'getCursorScreenPoint') return {'x': 0.0, 'y': 0.0};
      if (call.method == 'getAllDisplays') return {'displays': [display]};
      return display;
    });
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'setSkipTaskbar') {
        skipTaskbar = call.arguments['isSkipTaskbar'] as bool;
      }
      if (call.method.startsWith('is')) return false;
      if (call.method == 'setBounds') {
        final args = call.arguments;
        bounds = Rect.fromLTWH(
            args['x'] ?? bounds.left,
            args['y'] ?? bounds.top,
            args['width'] ?? bounds.width,
            args['height'] ?? bounds.height);
      }
      if (call.method == 'show' ||
          call.method == 'focus' ||
          (call.method == 'setOpacity' && call.arguments['opacity'] == 1)) {
        visibleBounds.add(bounds);
        visibleTaskbarEntries.add(!skipTaskbar);
      }
      return null;
    });
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      messenger.setMockMethodCallHandler(screenChannel, null);
    });

    await showCmWindow(isStartup: true);

    expect(visibleBounds, isNotEmpty);
    expect(visibleTaskbarEntries, everyElement(!(isWindows || isLinux)));
    expect(visibleBounds,
        everyElement(const Rect.fromLTWH(1584, 800, 336, 280)));
  });

  test('incoming voice call expands CM and keeps the call visible', () async {
    binding.option = 'Y';
    final oldDesktopType = desktopType;
    final oldIsTest = isTest;
    isTest = true;
    desktopType = DesktopType.cm;
    await initGlobalFFI();
    final client =
        Client(1, true, false, false, 'Peer', '123456789', true, true, true);
    gFFI.serverModel.clients.add(client);
    cmWindowDock.value = true;
    const channel = MethodChannel('window_manager');
    var bounds = const Rect.fromLTWH(1884, 864, 36, 72);
    final shown = Completer<void>();
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'getBounds') {
        return {
          'x': bounds.left,
          'y': bounds.top,
          'width': bounds.width,
          'height': bounds.height
        };
      }
      if (call.method == 'setBounds') {
        final args = call.arguments;
        bounds =
            Rect.fromLTWH(args['x'], args['y'], args['width'], args['height']);
      }
      if (call.method == 'isMinimized') return false;
      if (call.method == 'show' && !shown.isCompleted) shown.complete();
      return null;
    });
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      gFFI.serverModel.clients.clear();
      cmWindowDock.value = false;
      desktopType = oldDesktopType;
      isTest = oldIsTest;
    });

    gFFI.serverModel.updateVoiceCallState({
      'client': jsonEncode(client.toJson()..['incoming_voice_call'] = true),
    });
    await shown.future.timeout(const Duration(seconds: 2));
    expect(await windowManager.getBounds(),
        const Rect.fromLTWH(1584, 684, 336, 316));
    await cmWindowDock.dock().timeout(const Duration(seconds: 2));
    expect(await windowManager.getBounds(),
        const Rect.fromLTWH(1584, 684, 336, 316));
  });

  testWidgets('moved CM docks at its screen work area and expands in place',
      (tester) async {
    binding.option = 'Y';
    final oldDesktopType = desktopType;
    final oldIsTest = isTest;
    isTest = true;
    desktopType = DesktopType.cm;
    await initGlobalFFI();
    gFFI.serverModel.clients.add(
        Client(1, true, false, false, 'Peer', '123456789', true, true, true));
    final dock = cmWindowDock;
    var bounds = const Rect.fromLTWH(-1000, 200, 336, 280);
    // A frameless Windows window still has an OS minimum tracking width.
    var minimumWidth = isWindows ? 132.0 : 0.0;
    const hostChannel = MethodChannel('org.rustdesk.rustdesk/host');
    messenger.setMockMethodCallHandler(hostChannel, (call) async {
      if (call.method == 'setCmBannerWindow') {
        minimumWidth = 0;
        return null;
      }
      throw UnsupportedError(call.method);
    });
    addTearDown(() => messenger.setMockMethodCallHandler(hostChannel, null));
    const channel = MethodChannel('window_manager');
    const screenChannel = MethodChannel('flutter/windowsize');
    final scale = isWindows ? 2.0 : 1.0;
    messenger.setMockMethodCallHandler(screenChannel, (call) async => {
          'frame': [-1000.0, 200.0, 336.0, 280.0],
          'scaleFactor': 2.0,
          'screen': {
            'frame': [-1600 * scale, 0.0, 1600 * scale, 1080 * scale],
            'visibleFrame': [-1600 * scale, 0.0, 1560 * scale, 1040 * scale],
            'scaleFactor': 2.0,
          }
        });
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'isMinimized') return false;
      if (call.method == 'getBounds') {
        return {
          'x': bounds.left,
          'y': bounds.top,
          'width': bounds.width,
          'height': bounds.height
        };
      }
      if (call.method == 'setBounds') {
        final args = call.arguments;
        final width = args['width'] as double;
        bounds = Rect.fromLTWH(args['x'], args['y'],
            width < minimumWidth ? minimumWidth : width, args['height']);
      }
      return null;
    });
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      messenger.setMockMethodCallHandler(screenChannel, null);
      gFFI.serverModel.clients.clear();
      dock.value = false;
      desktopType = oldDesktopType;
      isTest = oldIsTest;
    });
    for (var i = 0; i < 2; i++) {
      final collapsing = dock.dock();
      await tester.pump();
      await collapsing;
      expect(await windowManager.getBounds(),
          const Rect.fromLTWH(-76, 344, 36, 72));
      await dock.expand();
      expect(await windowManager.getBounds(),
          const Rect.fromLTWH(-376, 200, 336, 280));
    }

    await tester.pumpWidget(MaterialApp(
        theme: MyTheme.lightTheme,
        home: const CmWindowDockView(child: Text('Connection details'))));
    for (final state in ['Close', 'Accept', 'Incoming call']) {
      final client = gFFI.serverModel.clients.single;
      client.disconnected = state == 'Close';
      client.authorized = state != 'Accept';
      client.incomingVoiceCall = state == 'Incoming call';
      await dock.resizeExpanded(dock.expandedSize);
      final expanded = await windowManager.getBounds();
      await dock.dock();
      expect(await windowManager.getBounds(), expanded);
      await tester.tap(find.byIcon(Icons.chevron_right));
      await tester.pumpAndSettle();
      expect(await windowManager.getBounds(),
          const Rect.fromLTWH(-76, 344, 36, 72), reason: state);
      await tester.tap(find.byType(CmDockTab));
      await tester.pumpAndSettle();
      expect(await windowManager.getBounds(), expanded);
    }
  });

  testWidgets('all CM actions remain visible without scrolling', (tester) async {
    binding.option = 'Y';
    final oldDesktopType = desktopType;
    final oldIsTest = isTest;
    isTest = true;
    desktopType = DesktopType.cm;
    await initGlobalFFI();
    final model = gFFI.serverModel;
    final client =
        Client(1, false, false, false, 'Peer', '123456789', true, true, true);
    model.clients.add(client);
    model.tabController.add(TabInfo(
        key: '1', label: client.name, page: buildConnectionCard(client)));
    var bounds = const Rect.fromLTWH(1584, 720, 336, 280);
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = bounds.size;
    const channel = MethodChannel('window_manager');
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method.startsWith('is')) return false;
      if (call.method == 'getBounds') {
        return {
          'x': bounds.left,
          'y': bounds.top,
          'width': bounds.width,
          'height': bounds.height
        };
      }
      if (call.method == 'setBounds') {
        final args = call.arguments;
        bounds =
            Rect.fromLTWH(args['x'], args['y'], args['width'], args['height']);
        tester.view.physicalSize = bounds.size;
      }
      return null;
    });
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      tester.view.resetPhysicalSize();
      tester.view.resetDevicePixelRatio();
      model.clients.clear();
      desktopType = oldDesktopType;
      isTest = oldIsTest;
    });
    await tester.pumpWidget(GetMaterialApp(
        theme: MyTheme.lightTheme, home: const DesktopServerPage()));
    await tester.pumpAndSettle();

    model.setShowElevation(true);
    await tester.pumpAndSettle();
    expect(bounds, const Rect.fromLTWH(1584, 684, 336, 316));
    for (final label in ['Accept and Elevate', 'Accept', 'Cancel']) {
      expect(find.text(label).hitTestable(), findsOneWidget);
    }

    client.authorized = true;
    client.fromSwitch = true;
    model.updateVoiceCallState({
      'client': jsonEncode(client.toJson()..['incoming_voice_call'] = true)
    });
    await tester.pumpAndSettle();
    expect(bounds, const Rect.fromLTWH(1584, 612, 336, 388));
    for (final label in [
      'Accept',
      'Dismiss',
      'Switch Sides',
      'Elevate',
      'Disconnect'
    ]) {
      expect(find.text(label).hitTestable(), findsOneWidget);
    }

    client.fromSwitch = false;
    client.incomingVoiceCall = false;
    model.setShowElevation(false);
    await tester.pumpAndSettle();
    expect(bounds, const Rect.fromLTWH(1584, 720, 336, 280));
    expect(find.text('Disconnect').hitTestable(), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 500));
  });

  testWidgets(
      'the app tab opens CM and the side arrow folds it without tooltips',
      (tester) async {
    var docked = true;
    await tester.pumpWidget(MaterialApp(
      theme: MyTheme.lightTheme,
      home: StatefulBuilder(
        builder: (context, setState) => Align(
            alignment: Alignment.bottomRight,
            child: SizedBox.fromSize(
              size: docked ? CmWindowDock.size : const Size(336, 280),
              child: CmDockLayout(
                docked: docked,
                contentSize: CmWindowDock.compactSize,
                onToggle: () => setState(() => docked = !docked),
                child: const Material(child: TextField()),
              ),
            )),
      ),
    ));

    await tester.pumpAndSettle();
    expect(find.image(const AssetImage('assets/icon.png')), findsOneWidget);
    expect(find.byType(Tooltip), findsNothing);
    expect(tester.takeException(), isNull);
    expect(find.byType(TextField), findsNothing);
    await tester.tapAt(
        tester.getTopLeft(find.byType(CmDockTab)) + const Offset(18, 58));
    await tester.pumpAndSettle();
    expect(find.byIcon(Icons.chevron_right), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(tester.getTopLeft(find.byType(TextField)).dx,
        tester.getTopLeft(find.byType(CmDockTab)).dx + 36);
    await tester.enterText(find.byType(TextField), 'Connection details');
    await tester.tap(find.byIcon(Icons.chevron_right));
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsNothing);
    await tester.tap(find.byType(CmDockTab));
    await tester.pumpAndSettle();
    expect(find.text('Connection details'), findsOneWidget);
    expect(find.byType(Tooltip), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
