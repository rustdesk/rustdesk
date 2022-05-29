import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'mobile/pages/settings_page.dart';
import 'models/model.dart';

int? windowId;

Future<Null> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  await FFI.ffiModel.init();
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
      default:
        break;
    }
  } else {
    // main window
    await windowManager.ensureInitialized();
    // start service
    FFI.serverModel.startService();
    WindowOptions windowOptions = WindowOptions(
      size: Size(1280, 720),
      center: true,
      backgroundColor: Colors.transparent,
      skipTaskbar: false,
      titleBarStyle: TitleBarStyle.normal,
    );
    windowManager.waitUntilReadyToShow(windowOptions, () async {
      await windowManager.show();
      await windowManager.focus();
    });

    runApp(App());
  }
}

class App extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    // final analytics = FirebaseAnalytics.instance;
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: FFI.ffiModel),
        ChangeNotifierProvider.value(value: FFI.imageModel),
        ChangeNotifierProvider.value(value: FFI.cursorModel),
        ChangeNotifierProvider.value(value: FFI.canvasModel),
      ],
      child: MaterialApp(
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
