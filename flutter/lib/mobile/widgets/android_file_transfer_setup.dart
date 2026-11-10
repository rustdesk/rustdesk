import 'package:flutter/material.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../utils/android_documents.dart';

const _setupOption = 'android-file-transfer-folder-setup';
const _folderChoicePadding = EdgeInsets.fromLTRB(12, 12, 12, 0);
const _folderChoiceSpacing = 4.0;

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
  final choiceStyle = OutlinedButton.styleFrom(
    alignment: AlignmentDirectional.centerStart,
    minimumSize: const Size(0, kMinInteractiveDimension),
  );
  return CustomAlertDialog(
    title: Text(translate('Transfer file')),
    content: Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(translate('Choose which folder remote devices can access.')),
        Padding(
          padding: _folderChoicePadding,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              OutlinedButton(
                style: choiceStyle,
                onPressed: () => close(_FolderChoice.workspace),
                child: Text(translate('RustDesk storage')),
              ),
              const SizedBox(height: _folderChoiceSpacing),
              OutlinedButton(
                style: choiceStyle,
                onPressed: () => close(_FolderChoice.other),
                child: Text('${translate('Other folders')}…'),
              ),
            ],
          ),
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
