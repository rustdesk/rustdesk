import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'desktop/pages/server_page.dart';
import 'models/server_model.dart';

/// -t lib/cm_main.dart to test cm
void main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();
  await windowManager.setSize(Size(400, 600));
  await windowManager.setAlignment(Alignment.topRight);
  await initEnv(kAppTypeMain);
  gFFI.serverModel.clients
      .add(Client(0, false, false, "UserA", "123123123", true, false, false));
  gFFI.serverModel.clients
      .add(Client(1, false, false, "UserB", "221123123", true, false, false));
  gFFI.serverModel.clients
      .add(Client(2, false, false, "UserC", "331123123", true, false, false));
  gFFI.serverModel.clients
      .add(Client(3, false, false, "UserD", "441123123", true, false, false));
  runApp(GetMaterialApp(
      debugShowCheckedModeBanner: false, home: DesktopServerPage()));
}
