import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/server_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

final testClients = [
  Client(0, false, false, "UserAAAAAA", "123123123", true, false, false),
  Client(1, false, false, "UserBBBBB", "221123123", true, false, false),
  Client(2, false, false, "UserC", "331123123", true, false, false),
  Client(3, false, false, "UserDDDDDDDDDDDd", "441123123", true, false, false)
];

/// flutter run -d {platform} -t test/cm_test.dart to test cm
void main(List<String> args) async {
  isTest = true;
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();
  await windowManager.setSize(const Size(400, 600));
  await windowManager.setAlignment(Alignment.topRight);
  await initEnv(kAppTypeMain);
  for (var client in testClients) {
    gFFI.serverModel.clients.add(client);
    gFFI.serverModel.tabController.add(TabInfo(
        key: client.id.toString(),
        label: client.name,
        closable: false,
        page: buildConnectionCard(client)));
  }

  runApp(GetMaterialApp(
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
      home: const DesktopServerPage()));
  WindowOptions windowOptions = getHiddenTitleBarWindowOptions(
      size: kConnectionManagerWindowSizeClosedChat);
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    // ensure initial window size to be changed
    await windowManager.setSize(kConnectionManagerWindowSizeClosedChat);
    await Future.wait([
      windowManager.setAlignment(Alignment.topRight),
      windowManager.focus(),
      windowManager.setOpacity(1)
    ]);
    // ensure
    windowManager.setAlignment(Alignment.topRight);
  });
}
