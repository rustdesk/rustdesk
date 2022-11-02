import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/desktop/pages/server_page.dart';
import 'package:flutter_hbb/desktop/pages/install_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_port_forward_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:window_manager/window_manager.dart';

// import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'consts.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'models/platform_model.dart';

int? windowId;
late List<String> bootArgs;

Future<void> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  debugPrint("launch args: $args");
  bootArgs = List.from(args);

  if (!isDesktop) {
    runMobileApp();
    return;
  }
  // main window
  if (args.isNotEmpty && args.first == 'multi_window') {
    windowId = int.parse(args[1]);
    stateGlobal.setWindowId(windowId!);
    WindowController.fromWindowId(windowId!).showTitleBar(false);
    final argument = args[2].isEmpty
        ? <String, dynamic>{}
        : jsonDecode(args[2]) as Map<String, dynamic>;
    int type = argument['type'] ?? -1;
    // to-do: No need to parse window id ?
    // Because stateGlobal.windowId is a global value.
    argument['windowId'] = windowId;
    WindowType wType = type.windowType;
    switch (wType) {
      case WindowType.RemoteDesktop:
        desktopType = DesktopType.remote;
        runRemoteScreen(argument);
        break;
      case WindowType.FileTransfer:
        desktopType = DesktopType.fileTransfer;
        runFileTransferScreen(argument);
        break;
      case WindowType.PortForward:
        desktopType = DesktopType.portForward;
        runPortForwardScreen(argument);
        break;
      default:
        break;
    }
  } else if (args.isNotEmpty && args.first == '--cm') {
    debugPrint("--cm started");
    desktopType = DesktopType.cm;
    await windowManager.ensureInitialized();
    runConnectionManagerScreen();
  } else if (args.contains('--install')) {
    runInstallPage();
  } else {
    desktopType = DesktopType.main;
    await windowManager.ensureInitialized();
    windowManager.setPreventClose(true);
    runMainApp(true);
  }
}

Future<void> initEnv(String appType) async {
  // global shared preference
  await Get.putAsync(() => SharedPreferences.getInstance());
  await platformFFI.init(appType);
  // global FFI, use this **ONLY** for global configuration
  // for convenience, use global FFI on mobile platform
  // focus on multi-ffi on desktop first
  await initGlobalFFI();
  // await Firebase.initializeApp();
  _registerEventHandler();
}

void runMainApp(bool startService) async {
  // register uni links
  initUniLinks();
  await initEnv(kAppTypeMain);
  // trigger connection status updater
  await bind.mainCheckConnectStatus();
  if (startService) {
    // await windowManager.ensureInitialized();
    gFFI.serverModel.startService();
  }
  runApp(App());
  // set window option
  WindowOptions windowOptions = getHiddenTitleBarWindowOptions();
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    restoreWindowPosition(WindowType.Main);
    await windowManager.show();
    await windowManager.focus();
    await windowManager.setOpacity(1);
  });
}

void runMobileApp() async {
  await initEnv(kAppTypeMain);
  if (isAndroid) androidChannelInit();
  runApp(App());
}

void runRemoteScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopRemote);
  runApp(RefreshWrapper(
    builder: (context) => GetMaterialApp(
      navigatorKey: globalKey,
      debugShowCheckedModeBanner: false,
      title: 'RustDesk - Remote Desktop',
      theme: MyTheme.lightTheme,
      darkTheme: MyTheme.darkTheme,
      themeMode: MyTheme.currentThemeMode(),
      home: DesktopRemoteScreen(
        params: argument,
      ),
      localizationsDelegates: const [
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: supportedLocales,
      navigatorObservers: const [
        // FirebaseAnalyticsObserver(analytics: analytics),
      ],
      builder: _keepScaleBuilder(),
    ),
  ));
}

void runFileTransferScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopFileTransfer);
  runApp(
    RefreshWrapper(
      builder: (context) => GetMaterialApp(
        navigatorKey: globalKey,
        debugShowCheckedModeBanner: false,
        title: 'RustDesk - File Transfer',
        theme: MyTheme.lightTheme,
        darkTheme: MyTheme.darkTheme,
        themeMode: MyTheme.currentThemeMode(),
        home: DesktopFileTransferScreen(params: argument),
        localizationsDelegates: const [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: supportedLocales,
        navigatorObservers: const [
          // FirebaseAnalyticsObserver(analytics: analytics),
        ],
        builder: _keepScaleBuilder(),
      ),
    ),
  );
}

void runPortForwardScreen(Map<String, dynamic> argument) async {
  await initEnv(kAppTypeDesktopPortForward);
  runApp(
    RefreshWrapper(builder: (context) {
      return GetMaterialApp(
        navigatorKey: globalKey,
        debugShowCheckedModeBanner: false,
        title: 'RustDesk - Port Forward',
        theme: MyTheme.lightTheme,
        darkTheme: MyTheme.darkTheme,
        themeMode: MyTheme.currentThemeMode(),
        home: DesktopPortForwardScreen(params: argument),
        localizationsDelegates: const [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: supportedLocales,
        navigatorObservers: const [
          // FirebaseAnalyticsObserver(analytics: analytics),
        ],
        builder: _keepScaleBuilder(),
      );
    }),
  );
}

void runConnectionManagerScreen() async {
  await initEnv(kAppTypeMain);
  // initialize window
  WindowOptions windowOptions =
      getHiddenTitleBarWindowOptions(size: kConnectionManagerWindowSize);
  runApp(RefreshWrapper(builder: (context) {
    return GetMaterialApp(
        debugShowCheckedModeBanner: false,
        theme: MyTheme.lightTheme,
        darkTheme: MyTheme.darkTheme,
        themeMode: MyTheme.currentThemeMode(),
        localizationsDelegates: const [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: supportedLocales,
        home: const DesktopServerPage(),
        builder: _keepScaleBuilder());
  }));
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    // ensure initial window size to be changed
    await windowManager.setSize(kConnectionManagerWindowSize);
    await Future.wait([
      windowManager.setAlignment(Alignment.topRight),
      windowManager.focus(),
      windowManager.setOpacity(1)
    ]);
    // ensure
    windowManager.setAlignment(Alignment.topRight);
  });
}

void runInstallPage() async {
  await windowManager.ensureInitialized();
  await initEnv(kAppTypeMain);
  runApp(RefreshWrapper(
    builder: (context) => GetMaterialApp(
        debugShowCheckedModeBanner: false,
        theme: MyTheme.lightTheme,
        themeMode: ThemeMode.light,
        localizationsDelegates: const [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: supportedLocales,
        home: const InstallPage(),
        builder: _keepScaleBuilder()),
  ));
  windowManager.waitUntilReadyToShow(
      WindowOptions(size: Size(800, 600), center: true), () async {
    windowManager.show();
    windowManager.focus();
    windowManager.setOpacity(1);
    windowManager.setAlignment(Alignment.center); // ensure
  });
}

WindowOptions getHiddenTitleBarWindowOptions({Size? size}) {
  return WindowOptions(
    size: size,
    center: false,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    titleBarStyle: TitleBarStyle.hidden,
  );
}

class App extends StatefulWidget {
  @override
  State<App> createState() => _AppState();
}

class _AppState extends State<App> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.window.onPlatformBrightnessChanged = () {
      final userPreference = MyTheme.getThemeModePreference();
      if (userPreference != ThemeMode.system) return;
      WidgetsBinding.instance.handlePlatformBrightnessChanged();
      final systemIsDark =
          WidgetsBinding.instance.platformDispatcher.platformBrightness ==
              Brightness.dark;
      final ThemeMode to;
      if (systemIsDark) {
        to = ThemeMode.dark;
      } else {
        to = ThemeMode.light;
      }
      Get.changeThemeMode(to);
      if (desktopType == DesktopType.main) {
        bind.mainChangeTheme(dark: to.toShortString());
      }
    };
  }

  @override
  Widget build(BuildContext context) {
    // final analytics = FirebaseAnalytics.instance;
    return RefreshWrapper(builder: (context) {
      return MultiProvider(
        providers: [
          // global configuration
          // use session related FFI when in remote control or file transfer page
          ChangeNotifierProvider.value(value: gFFI.ffiModel),
          ChangeNotifierProvider.value(value: gFFI.imageModel),
          ChangeNotifierProvider.value(value: gFFI.cursorModel),
          ChangeNotifierProvider.value(value: gFFI.canvasModel),
        ],
        child: GetMaterialApp(
          navigatorKey: globalKey,
          debugShowCheckedModeBanner: false,
          title: 'RustDesk',
          theme: MyTheme.lightTheme,
          darkTheme: MyTheme.darkTheme,
          themeMode: MyTheme.currentThemeMode(),
          home: isDesktop
              ? const DesktopTabPage()
              : !isAndroid
                  ? WebHomePage()
                  : HomePage(),
          navigatorObservers: const [
            // FirebaseAnalyticsObserver(analytics: analytics),
          ],
          localizationsDelegates: const [
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          supportedLocales: supportedLocales,
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
    });
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

_registerEventHandler() {
  if (isDesktop && desktopType != DesktopType.main) {
    platformFFI.registerEventHandler('theme', 'theme', (evt) async {
      String? dark = evt['dark'];
      if (dark != null) {
        MyTheme.changeDarkMode(MyTheme.themeModeFromString(dark));
      }
    });
    platformFFI.registerEventHandler('language', 'language', (_) async {
      reloadAllWindows();
    });
  }
}
