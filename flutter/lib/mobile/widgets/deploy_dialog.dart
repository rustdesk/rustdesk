import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

const _deployDialogTag = 'android-deploy-device';

void showDeployPromptDialog() {
  gFFI.dialogManager.dismissByTag(_deployDialogTag);
  gFFI.dialogManager.show<bool>((setState, close, context) {
    submit() => close(true);
    return CustomAlertDialog(
      title: Text(translate("Deploy")),
      content: Text(translate("server_requires_deployment_tip")),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  }, tag: _deployDialogTag).then((deploy) {
    if (deploy == true) {
      showDeployDialog();
    }
  });
}

void showDeployDialog() {
  gFFI.dialogManager.dismissByTag(_deployDialogTag);
  final tokenController = TextEditingController();
  final idController = TextEditingController();
  var errorText = "";
  var isInProgress = false;
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      if (isInProgress) return;
      final token = tokenController.text.trim();
      if (token.isEmpty) {
        setState(() {
          errorText = translate("token is required!");
        });
        return;
      }
      setState(() {
        errorText = "";
        isInProgress = true;
      });
      String res;
      try {
        res = await bind.mainDeployDevice(
            token: token, id: idController.text.trim());
      } catch (e) {
        setState(() {
          errorText = translate(e.toString());
          isInProgress = false;
        });
        return;
      }
      if (res.isEmpty) {
        close();
        await gFFI.serverModel.fetchID();
        showToast(translate("Successful"));
      } else {
        setState(() {
          errorText = translate(res.toString());
          isInProgress = false;
        });
      }
    }

    return CustomAlertDialog(
      title: Text(translate("Deploy")),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: tokenController,
            decoration: InputDecoration(labelText: translate("API Token")),
            obscureText: true,
            enableSuggestions: false,
            autocorrect: false,
            autofocus: true,
          ).workaroundFreezeLinuxMint(),
          TextField(
            controller: idController,
            decoration:
                InputDecoration(labelText: translate("Custom ID (optional)")),
          ).workaroundFreezeLinuxMint(),
          if (errorText.isNotEmpty)
            Align(
              alignment: Alignment.centerLeft,
              child: SelectableText(
                errorText,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.error,
                  fontSize: 12,
                ),
              ).paddingOnly(top: 8),
            ),
          if (isInProgress) const LinearProgressIndicator().paddingOnly(top: 8),
        ],
      ),
      actions: [
        dialogButton("Cancel",
            onPressed: isInProgress ? null : close, isOutline: true),
        dialogButton("OK", onPressed: isInProgress ? null : submit),
      ],
      onSubmit: submit,
      onCancel: isInProgress ? null : close,
    );
  }, tag: _deployDialogTag);
}
