import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

void changeIdDialog() {
  var newId = "";
  var msg = "";
  var isInProgress = false;
  TextEditingController controller = TextEditingController();
  gFFI.dialogManager.show((setState, close) {
    submit() async {
      debugPrint("onSubmit");
      newId = controller.text.trim();
      setState(() {
        msg = "";
        isInProgress = true;
        bind.mainChangeId(newId: newId);
      });

      var status = await bind.mainGetAsyncStatus();
      while (status == " ") {
        await Future.delayed(const Duration(milliseconds: 100));
        status = await bind.mainGetAsyncStatus();
      }
      if (status.isEmpty) {
        // ok
        close();
        return;
      }
      setState(() {
        isInProgress = false;
        msg = translate(status);
      });
    }

    return CustomAlertDialog(
      title: Text(translate("Change ID")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("id_change_tip")),
          const SizedBox(
            height: 12.0,
          ),
          TextField(
            decoration: InputDecoration(
                border: const OutlineInputBorder(),
                errorText: msg.isEmpty ? null : translate(msg)),
            inputFormatters: [
              LengthLimitingTextInputFormatter(16),
              // FilteringTextInputFormatter(RegExp(r"[a-zA-z][a-zA-z0-9\_]*"), allow: true)
            ],
            maxLength: 16,
            controller: controller,
            focusNode: FocusNode()..requestFocus(),
          ),
          const SizedBox(
            height: 4.0,
          ),
          Offstage(
              offstage: !isInProgress, child: const LinearProgressIndicator())
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void changeWhiteList({Function()? callback}) async {
  var newWhiteList = (await bind.mainGetOption(key: 'whitelist')).split(',');
  var newWhiteListField = newWhiteList.join('\n');
  var controller = TextEditingController(text: newWhiteListField);
  var msg = "";
  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("IP Whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          const SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                    maxLines: null,
                    decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: controller,
                    focusNode: FocusNode()..requestFocus()),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          Offstage(
              offstage: !isInProgress, child: const LinearProgressIndicator())
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("Clear", onPressed: () async {
          await bind.mainSetOption(key: 'whitelist', value: '');
          callback?.call();
          close();
        }, isOutline: true),
        dialogButton(
          "OK",
          onPressed: () async {
            setState(() {
              msg = "";
              isInProgress = true;
            });
            newWhiteListField = controller.text.trim();
            var newWhiteList = "";
            if (newWhiteListField.isEmpty) {
              // pass
            } else {
              final ips = newWhiteListField.trim().split(RegExp(r"[\s,;\n]+"));
              // test ip
              final ipMatch = RegExp(
                  r"^(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)(\/([1-9]|[1-2][0-9]|3[0-2])){0,1}$");
              final ipv6Match = RegExp(
                  r"^(((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*::((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*|((?:[0-9A-Fa-f]{1,4}))((?::[0-9A-Fa-f]{1,4})){7})(\/([1-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8])){0,1}$");
              for (final ip in ips) {
                if (!ipMatch.hasMatch(ip) && !ipv6Match.hasMatch(ip)) {
                  msg = "${translate("Invalid IP")} $ip";
                  setState(() {
                    isInProgress = false;
                  });
                  return;
                }
              }
              newWhiteList = ips.join(',');
            }
            await bind.mainSetOption(key: 'whitelist', value: newWhiteList);
            callback?.call();
            close();
          },
        ),
      ],
      onCancel: close,
    );
  });
}

Future<String> changeDirectAccessPort(
    String currentIP, String currentPort) async {
  final controller = TextEditingController(text: currentPort);
  await gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("Change Local Port")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                    maxLines: null,
                    keyboardType: TextInputType.number,
                    decoration: InputDecoration(
                        hintText: '21118',
                        isCollapsed: true,
                        prefix: Text('$currentIP : '),
                        suffix: IconButton(
                            padding: EdgeInsets.zero,
                            icon: const Icon(Icons.clear, size: 16),
                            onPressed: () => controller.clear())),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                    ],
                    controller: controller,
                    focusNode: FocusNode()..requestFocus()),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: 'direct-access-port', value: controller.text);
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return controller.text;
}
