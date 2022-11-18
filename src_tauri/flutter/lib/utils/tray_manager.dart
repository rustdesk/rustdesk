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
  if (Platform.isMacOS || Platform.isWindows) {
    await trayManager.setToolTip("rustdesk");
  }
  if (Platform.isMacOS || Platform.isLinux) {
    await trayManager.setTitle("rustdesk");
  }
  await trayManager
      .setIcon(Platform.isWindows ? "assets/logo.ico" : "assets/logo.png");
  await trayManager.setContextMenu(Menu(items: items));
}

Future<void> destoryTray() async {
  return trayManager.destroy();
}
