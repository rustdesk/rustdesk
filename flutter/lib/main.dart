import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:get/route_manager.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';

// import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'consts.dart';
import 'models/platform_model.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'mobile/pages/settings_page.dart';

int? windowId;

Future<Null> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();

  if (!isDesktop) {
    runMainApp(false);
    return;
  }
  // main window
  if (args.isNotEmpty && args.first == 'multi_window') {
    windowId = int.parse(args[1]);
    final argument = args[2].isEmpty
        ? Map<String, dynamic>()
        : jsonDecode(args[2]) as Map<String, dynamic>;
    int type = argument['type'] ?? -1;
    argument['windowId'] = windowId;
    WindowType wType = type.windowType;
    switch (wType) {
      case WindowType.RemoteDesktop:
        runRemoteScreen(argument);
        break;
      case WindowType.FileTransfer:
        runFileTransferScreen(argument);
        break;
      default:
        break;
    }
  } else {
    await windowManager.ensureInitialized();
    windowManager.setPreventClose(true);
    runMainApp(true);
  }
}

ThemeData getCurrentTheme() {
  return isDarkTheme() ? MyTheme.darkTheme : MyTheme.darkTheme;
}

Future<void> initEnv(String appType) async {
  await platformFFI.init(appType);
  // global FFI, use this **ONLY** for global configuration
  // for convenience, use global FFI on mobile platform
  // focus on multi-ffi on desktop first
  await initGlobalFFI();
  // await Firebase.initializeApp();
  if (isAndroid) {
    toAndroidChannelInit();
  }
  refreshCurrentUser();
}

void runMainApp(bool startService) async {
  await initEnv(kAppTypeMain);
  if (startService) {
    // await windowManager.ensureInitialized();
    // disable tray
    // initTray();
    gFFI.serverModel.startService();
  }
  runApp(App());
}

void runRemoteScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopRemote);
  runApp(GetMaterialApp(
    theme: getCurrentTheme(),
    home: DesktopRemoteScreen(
      params: argument,
    ),
  ));
}

void runFileTransferScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopFileTransfer);
  runApp(GetMaterialApp(
      theme: getCurrentTheme(),
      home: DesktopFileTransferScreen(params: argument)));
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
          theme: getCurrentTheme(),
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
