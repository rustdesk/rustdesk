import 'package:flutter/material.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../utils/android_documents.dart';

const _setupOption = 'android-file-transfer-folder-setup';

enum _FolderChoice { workspace, other, skip }

Future<bool> showAndroidFileTransferSetup(FFI ffi) async {
  if (bind.getLocalFlutterOption(k: _setupOption) == 'Y' ||
      !_canConfigureSharedFolder(ffi)) {
    return true;
  }

  try {
    final choice = await ffi.dialogManager.show<_FolderChoice>(
      (_, close, __) => _setupDialog(close),
      backDismiss: true,
      forceGlobal: true,
    );
    if (choice == null) return false;
    switch (choice) {
      case _FolderChoice.workspace:
        await AndroidDocuments.workspace(share: true);
        break;
      case _FolderChoice.other:
        if (await AndroidDocuments.selectFolder(share: true) == null) {
          return false;
        }
        break;
      case _FolderChoice.skip:
        break;
    }
    await bind.setLocalFlutterOption(k: _setupOption, v: 'Y');
    return true;
  } catch (error) {
    showToast(error.toString());
    return false;
  }
}

bool _canConfigureSharedFolder(FFI ffi) =>
    !bind.isOutgoingOnly() &&
    !bind.isDisableSettings() &&
    bind.mainGetBuildinOption(key: kOptionHideSecuritySetting) != 'Y' &&
    (!ffi.serverModel.clients.any((client) => !client.disconnected) ||
        option2bool(
            kOptionEnablePermChangeInAcceptWindow,
            bind.mainGetBuildinOption(
                key: kOptionEnablePermChangeInAcceptWindow)));

CustomAlertDialog _setupDialog(void Function([dynamic]) close) {
  final settingsPath =
      '${translate('Settings')} → ${translate('Transfer file')} → ${translate('Change Path')}';
  return CustomAlertDialog(
    title: Text(translate('Transfer file')),
    content: Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(translate(
            'Choose which folder remote devices can access. You can change it later in {$settingsPath}.')),
        ListTile(
          title: Text(translate('RustDesk storage')),
          onTap: () => close(_FolderChoice.workspace),
        ),
        ListTile(
          title: Text('${translate('Other folders')}…'),
          onTap: () => close(_FolderChoice.other),
        ),
      ],
    ),
    actions: [
      TextButton(
        onPressed: () => close(_FolderChoice.skip),
        child: Text(translate('Skip')),
      ),
    ],
    onCancel: close,
  );
}
