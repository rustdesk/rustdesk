import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

import 'desktop/pages/server_page.dart';

/// -t lib/cm_main.dart to test cm
void main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();
  await windowManager.setSize(Size(400, 600));
  await windowManager.setAlignment(Alignment.topRight);
  await initEnv(kAppTypeConnectionManager);
  runApp(GetMaterialApp(theme: getCurrentTheme(), home: DesktopServerPage()));
}
