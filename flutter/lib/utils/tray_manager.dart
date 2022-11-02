import 'dart:io';

import 'package:tray_manager/tray_manager.dart';

import '../common.dart';

const kTrayItemShowKey = "show";
const kTrayItemQuitKey = "quit";

Future<void> initTray({List<MenuItem>? extra_item}) async {
  List<MenuItem> items = [
    MenuItem(key: kTrayItemShowKey, label: translate("Show RustDesk")),
    MenuItem.separator(),
    MenuItem(key: kTrayItemQuitKey, label: translate("Quit")),
  ];
  if (extra_item != null) {
    items.insertAll(0, extra_item);
  }
  await Future.wait([
    trayManager
        .setIcon(Platform.isWindows ? "assets/logo.ico" : "assets/logo.png"),
    trayManager.setContextMenu(Menu(items: items)),
    trayManager.setTitle("rustdesk")
  ]);
  if (Platform.isMacOS || Platform.isWindows) {
    await trayManager.setToolTip("rustdesk");
  }
}

Future<void> destoryTray() async {
  return trayManager.destroy();
}
