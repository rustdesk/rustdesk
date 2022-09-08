import 'package:flutter/material.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';

/// Connect to a peer with [id].
/// If [isFileTransfer], starts a session only for file transfer.
/// If [isTcpTunneling], starts a session only for tcp tunneling.
/// If [isRDP], starts a session only for rdp.
void connect(BuildContext context, String id,
    {bool isFileTransfer = false,
    bool isTcpTunneling = false,
    bool isRDP = false}) async {
  if (id == '') return;
  id = id.replaceAll(' ', '');
  assert(!(isFileTransfer && isTcpTunneling && isRDP),
      "more than one connect type");

  FocusScopeNode currentFocus = FocusScope.of(context);
  if (isFileTransfer) {
    await rustDeskWinManager.newFileTransfer(id);
  } else if (isTcpTunneling || isRDP) {
    await rustDeskWinManager.newPortForward(id, isRDP);
  } else {
    await rustDeskWinManager.newRemoteDesktop(id);
  }
  if (!currentFocus.hasPrimaryFocus) {
    currentFocus.unfocus();
  }
}
