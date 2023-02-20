import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

abstract class ValidationRule {
  String get name;
  bool validate(String value);
}

class LengthRangeValidationRule extends ValidationRule {
  final int _min;
  final int _max;

  LengthRangeValidationRule(this._min, this._max);

  @override
  String get name => translate('length %min% to %max%')
      .replaceAll('%min%', _min.toString())
      .replaceAll('%max%', _max.toString());

  @override
  bool validate(String value) {
    return value.length >= _min && value.length <= _max;
  }
}

class RegexValidationRule extends ValidationRule {
  final String _name;
  final RegExp _regex;

  RegexValidationRule(this._name, this._regex);

  @override
  String get name => translate(_name);

  @override
  bool validate(String value) {
    return value.isNotEmpty ? value.contains(_regex) : false;
  }
}

void changeIdDialog() {
  var newId = "";
  var msg = "";
  var isInProgress = false;
  TextEditingController controller = TextEditingController();
  final RxString rxId = controller.text.trim().obs;

  final rules = [
    RegexValidationRule('starts with a letter', RegExp(r'^[a-zA-Z]')),
    LengthRangeValidationRule(6, 16),
    RegexValidationRule('allowed characters', RegExp(r'^\w*$'))
  ];

  gFFI.dialogManager.show((setState, close) {
    submit() async {
      debugPrint("onSubmit");
      newId = controller.text.trim();

      final Iterable violations = rules.where((r) => !r.validate(newId));
      if (violations.isNotEmpty) {
        setState(() {
          msg =
              '${translate('Prompt')}: ${violations.map((r) => r.name).join(', ')}';
        });
        return;
      }

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
        msg = '${translate('Prompt')}: ${translate(status)}';
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
                labelText: translate('Your new ID'),
                border: const OutlineInputBorder(),
                errorText: msg.isEmpty ? null : translate(msg),
                suffixText: '${rxId.value.length}/16',
                suffixStyle: const TextStyle(fontSize: 12, color: Colors.grey)),
            inputFormatters: [
              LengthLimitingTextInputFormatter(16),
              // FilteringTextInputFormatter(RegExp(r"[a-zA-z][a-zA-z0-9\_]*"), allow: true)
            ],
            controller: controller,
            autofocus: true,
            onChanged: (value) {
              setState(() {
                rxId.value = value.trim();
                msg = '';
              });
            },
          ),
          const SizedBox(
            height: 8.0,
          ),
          Obx(() => Wrap(
                runSpacing: 8,
                spacing: 4,
                children: rules.map((e) {
                  var checked = e.validate(rxId.value);
                  return Chip(
                      label: Text(
                        e.name,
                        style: TextStyle(
                            color: checked
                                ? const Color(0xFF0A9471)
                                : Color.fromARGB(255, 198, 86, 157)),
                      ),
                      backgroundColor: checked
                          ? const Color(0xFFD0F7ED)
                          : Color.fromARGB(255, 247, 205, 232));
                }).toList(),
              )),
          const SizedBox(
            height: 8.0,
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
                    autofocus: true),
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
                    autofocus: true),
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
