import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/desktop/pages/server_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_port_forward_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';

// import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'consts.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'mobile/pages/settings_page.dart';
import 'models/platform_model.dart';

int? windowId;

Future<Null> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  print("launch args: $args");

  if (!isDesktop) {
    runMobileApp();
    return;
  }
  // main window
  if (args.isNotEmpty && args.first == 'multi_window') {
    windowId = int.parse(args[1]);
    WindowController.fromWindowId(windowId!).showTitleBar(false);
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
      case WindowType.PortForward:
        runPortForwardScreen(argument);
        break;
      default:
        break;
    }
  } else if (args.isNotEmpty && args.first == '--cm') {
    print("--cm started");
    await windowManager.ensureInitialized();
    runConnectionManagerScreen();
  } else {
    await windowManager.ensureInitialized();
    windowManager.setPreventClose(true);
    runMainApp(true);
  }
}

ThemeData getCurrentTheme() {
  return isDarkTheme() ? MyTheme.darkTheme : MyTheme.lightTheme;
}

Future<void> initEnv(String appType) async {
  await platformFFI.init(appType);
  // global FFI, use this **ONLY** for global configuration
  // for convenience, use global FFI on mobile platform
  // focus on multi-ffi on desktop first
  await initGlobalFFI();
  // await Firebase.initializeApp();
  refreshCurrentUser();
}

void runMainApp(bool startService) async {
  await initEnv(kAppTypeMain);
  // trigger connection status updater
  await bind.mainCheckConnectStatus();
  if (startService) {
    // await windowManager.ensureInitialized();
    // disable tray
    // initTray();
    gFFI.serverModel.startService();
  }
  runApp(App());
  // set window option
  WindowOptions windowOptions =
      getHiddenTitleBarWindowOptions(const Size(1280, 720));
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    await windowManager.focus();
  });
}

void runMobileApp() async {
  await initEnv(kAppTypeMain);
  if (isAndroid) androidChannelInit();
  runApp(App());
}

void runRemoteScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopRemote);
  runApp(GetMaterialApp(
    navigatorKey: globalKey,
    debugShowCheckedModeBanner: false,
    title: 'RustDesk - Remote Desktop',
    theme: getCurrentTheme(),
    home: DesktopRemoteScreen(
      params: argument,
    ),
    navigatorObservers: [
      // FirebaseAnalyticsObserver(analytics: analytics),
    ],
    builder: _keepScaleBuilder(),
  ));
}

void runFileTransferScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopFileTransfer);
  runApp(
    GetMaterialApp(
      navigatorKey: globalKey,
      debugShowCheckedModeBanner: false,
      title: 'RustDesk - File Transfer',
      theme: getCurrentTheme(),
      home: DesktopFileTransferScreen(params: argument),
      navigatorObservers: [
        // FirebaseAnalyticsObserver(analytics: analytics),
      ],
      builder: _keepScaleBuilder(),
    ),
  );
}

void runPortForwardScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopPortForward);
  runApp(
    GetMaterialApp(
      navigatorKey: globalKey,
      debugShowCheckedModeBanner: false,
      title: 'RustDesk - Port Forward',
      theme: getCurrentTheme(),
      home: DesktopPortForwardScreen(params: argument),
      navigatorObservers: [
        // FirebaseAnalyticsObserver(analytics: analytics),
      ],
      builder: _keepScaleBuilder(),
    ),
  );
}

void runConnectionManagerScreen() async {
  // initialize window
  WindowOptions windowOptions = getHiddenTitleBarWindowOptions(Size(300, 400));
  await Future.wait([
    initEnv(kAppTypeMain),
    windowManager.waitUntilReadyToShow(windowOptions, () async {
      await windowManager.setAlignment(Alignment.topRight);
      await windowManager.show();
      await windowManager.focus();
    })
  ]);
  runApp(GetMaterialApp(
      debugShowCheckedModeBanner: false,
      theme: getCurrentTheme(),
      home: DesktopServerPage(),
      builder: _keepScaleBuilder()));
}

WindowOptions getHiddenTitleBarWindowOptions(Size size) {
  return WindowOptions(
    size: size,
    center: true,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    titleBarStyle: TitleBarStyle.hidden,
  );
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
            ? const DesktopTabPage()
            : !isAndroid
                ? WebHomePage()
                : HomePage(),
        navigatorObservers: [
          // FirebaseAnalyticsObserver(analytics: analytics),
        ],
        builder: isAndroid
            ? (context, child) => AccessibilityListener(
                  child: MediaQuery(
                    data: MediaQuery.of(context).copyWith(
                      textScaleFactor: 1.0,
                    ),
                    child: child ?? Container(),
                  ),
                )
            : _keepScaleBuilder(),
      ),
    );
  }
}

_keepScaleBuilder() {
  return (BuildContext context, Widget? child) {
    return MediaQuery(
      data: MediaQuery.of(context).copyWith(
        textScaleFactor: 1.0,
      ),
      child: child ?? Container(),
    );
  };
}
