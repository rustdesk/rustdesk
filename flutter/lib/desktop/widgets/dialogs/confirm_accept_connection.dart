import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:uuid/uuid.dart';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/common/widgets/dialogs/widgets/title.dart';
import 'package:flutter_hbb/common/widgets/dialogs/widgets/buttons.dart';

class ClientCard extends StatelessWidget {
  final /*Client*/ TrustedClient client;

  ClientCard(this.client);

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
      decoration: BoxDecoration(
        color: Theme.of(context).brightness == Brightness.dark
            ? Color.fromARGB(135, 87, 87, 90)
            : Colors.grey[100],
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.grey),
      ),
      child: Row(
        children: [
          CircleAvatar(
            backgroundColor: str2color(client.name,
                Theme.of(context).brightness == Brightness.light ? 255 : 150),
            child: Text(client.name[0]),
          ).marginOnly(right: 15),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(
                  client.name,
                  style: const TextStyle(fontSize: 18),
                ).marginOnly(bottom: 8),
                Text(
                  client.peerId,
                  style: const TextStyle(fontSize: 12),
                )
              ]))
        ],
      ),
    );
  }
}

void showConfirmAcceptConnection(/*Client*/ TrustedClient client) {
  final OverlayDialogManager dialogManager = gFFI.dialogManager;
  final UuidValue sessionId = gFFI.sessionId;

  final double fontSizeNote = 13.00;
  final RxBool addToTrustedClients = false.obs;

  Widget SettingsNote = Container(
    // padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
    decoration: BoxDecoration(
        // color: MyTheme.currentThemeMode() == ThemeMode.dark
        //     ? Color.fromARGB(135, 87, 87, 90)
        //     : Colors.grey[100],
        // borderRadius: BorderRadius.circular(8),
        // border: Border.all(color: Colors.grey),
        ),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(Icons.info_outline_rounded, size: 20).marginOnly(right: 10),
        Expanded(
          child: Text(
            translate(
                'If you select this option and this person wants to connect again, this dialog will not be displayed again. You can change this setting at any time under "Settings > Security > Trusted clients/persons".'),
            style: TextStyle(
                fontSize: fontSizeNote, fontWeight: FontWeight.normal),
          ),
        )
      ],
    ),
  );

  Widget Option = Obx(
    () => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Checkbox(
          visualDensity: VisualDensity(horizontal: -4, vertical: -4),
          value: addToTrustedClients.value,
          onChanged: (bool? value) =>
              addToTrustedClients.value = !addToTrustedClients.value,
        ).marginOnly(right: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              InkWell(
                hoverColor: Colors.transparent,
                onTap: () =>
                    addToTrustedClients.value = !addToTrustedClients.value,
                child: Text(
                  translate('Add to trusted persons/clients'),
                ),
              ).marginOnly(bottom: 10),
              SettingsNote
            ],
          ).marginOnly(top: 3),
        ),
      ],
    ),
  );

  var content = Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text('You are about to grant this person access to your device.')
          .marginOnly(bottom: 16),
      ClientCard(client).marginOnly(bottom: 15),
      Option,
    ],
  );

  dialogManager.show(tag: '$sessionId-confirm-accept-connection',
      (setState, close, context) {
    void submit() {
      debugPrint(addToTrustedClients.value.toString());
    }

    return CustomAlertDialog(
      contentBoxConstraints: BoxConstraints(maxWidth: 450),
      title: dialogTitle(
        'dialog.confirm_accept_connection.title',
        icon: Icons.security_outlined,
      ),
      content: content,
      actions: [
        dialogCancelButton(onPressed: close),
        dialogSubmitButton(onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}
