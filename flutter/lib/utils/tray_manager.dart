import 'dart:io';

import 'package:tray_manager/tray_manager.dart';

import '../common.dart';

Future<void> initTray({List<MenuItem>? extra_item}) async {
  List<MenuItem> items = [
    MenuItem(key: "show", label: translate("show rustdesk")),
    MenuItem.separator(),
    MenuItem(key: "quit", label: translate("quit rustdesk")),
  ];
  if (extra_item != null) {
    items.insertAll(0, extra_item);
  }
  await Future.wait([
    trayManager
        .setIcon(Platform.isWindows ? "assets/logo.ico" : "assets/logo.png"),
    trayManager.setContextMenu(Menu(items: items)),
    trayManager.setToolTip("rustdesk"),
    trayManager.setTitle("rustdesk")
  ]);
}
