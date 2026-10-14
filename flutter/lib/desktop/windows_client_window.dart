import 'package:window_manager/window_manager.dart';

import '../models/server_model.dart';

Future<void> hideWindowsClientWindow() async {
  await windowManager.hide();
  await windowManager.setSkipTaskbar(true);
}

Future<void> showWindowsApprovalWindow(Iterable<Client> clients,
    {bool elevation = false}) async {
  if (!elevation && !clients.any((client) => client.needsLocalApproval)) return;
  if (await windowManager.isVisible() && !await windowManager.isMinimized()) {
    return;
  }
  if (await windowManager.isMinimized()) await windowManager.restore();
  await windowManager.show();
  await windowManager.setSkipTaskbar(false);
  await windowManager.focus();
}
