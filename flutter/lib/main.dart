import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/route_manager.dart';
import 'package:provider/provider.dart';

// import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'mobile/pages/settings_page.dart';

int? windowId;

Future<Null> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  // global FFI, use this **ONLY** for global configuration
  // for convenience, use global FFI on mobile platform
  // focus on multi-ffi on desktop first
  await initGlobalFFI();
  // await Firebase.initializeApp();
  if (isAndroid) {
    toAndroidChannelInit();
  }
  refreshCurrentUser();
  runRustDeskApp(args);
}

void runRustDeskApp(List<String> args) async {
  if (!isDesktop) {
    runApp(App());
    return;
  }
  // main window
  if (args.isNotEmpty && args.first == 'multi_window') {
    windowId = int.parse(args[1]);
    final argument = args[2].isEmpty
        ? Map<String, dynamic>()
        : jsonDecode(args[2]) as Map<String, dynamic>;
    int type = argument['type'] ?? -1;
    WindowType wType = type.windowType;
    switch (wType) {
      case WindowType.RemoteDesktop:
        runApp(DesktopRemoteScreen(
          params: argument,
        ));
        break;
      case WindowType.FileTransfer:
        runApp(DesktopFileTransferScreen(params: argument));
        break;
      default:
        break;
    }
  } else {
    // await windowManager.ensureInitialized();
    // disable tray
    // initTray();
    gFFI.serverModel.startService();
    runApp(App());
  }
}

class App extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    // final analytics = FirebaseAnalytics.instance;
    return MultiProvider(
      providers: [
        // global configuration
        // use session related FFI when in remote control or file transfer page
        ChangeNotifierProvider.value(value: gFFI.ffiModel),
        ChangeNotifierProvider.value(value: gFFI.imageModel),
        ChangeNotifierProvider.value(value: gFFI.cursorModel),
        ChangeNotifierProvider.value(value: gFFI.canvasModel),
        ChangeNotifierProvider.value(value: gFFI.abModel),
        ChangeNotifierProvider.value(value: gFFI.userModel),
      ],
      child: GetMaterialApp(
          navigatorKey: globalKey,
          debugShowCheckedModeBanner: false,
          title: 'RustDesk',
          theme: ThemeData(
            primarySwatch: Colors.blue,
            visualDensity: VisualDensity.adaptivePlatformDensity,
          ),
          home: isDesktop
              ? DesktopHomePage()
              : !isAndroid
                  ? WebHomePage()
                  : HomePage(),
          navigatorObservers: [
            // FirebaseAnalyticsObserver(analytics: analytics),
            FlutterSmartDialog.observer
          ],
          builder: FlutterSmartDialog.init(
              builder: isAndroid
                  ? (_, child) => AccessibilityListener(
                child: child,
              )
                  : null)),
    );
  }
}
