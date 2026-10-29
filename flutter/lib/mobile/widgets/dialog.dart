import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/api_server_warning.dart';
import 'package:flutter_hbb/common/widgets/setting_widgets.dart';
import 'package:flutter_hbb/common/widgets/toolbar.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

void _showSuccess() {
  showToast(translate("Successful"));
}

void setTemporaryPasswordLengthDialog(
    OverlayDialogManager dialogManager) async {
  List<String> lengths = ['6', '8', '10'];
  String length = await bind.mainGetOption(key: "temporary-password-length");
  var index = lengths.indexOf(length);
  if (index < 0) index = 0;
  length = lengths[index];
  dialogManager.show((setState, close, context) {
    setLength(newValue) {
      final oldValue = length;
      if (oldValue == newValue) return;
      setState(() {
        length = newValue;
      });
      bind.mainSetOption(key: "temporary-password-length", value: newValue);
      bind.mainUpdateTemporaryPassword();
      Future.delayed(Duration(milliseconds: 200), () {
        close();
        _showSuccess();
      });
    }

    return CustomAlertDialog(
      title: Text(translate("Set one-time password length")),
      content: Row(
          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
          children: lengths
              .map(
                (value) => Row(
                  children: [
                    Text(value),
                    Radio(
                        value: value, groupValue: length, onChanged: setLength),
                  ],
                ),
              )
              .toList()),
    );
  }, backDismiss: true, clickMaskDismiss: true);
}

void showServerSettings(OverlayDialogManager dialogManager,
    void Function(VoidCallback) setState) async {
  Map<String, dynamic> options = {};
  try {
    options = jsonDecode(await bind.mainGetOptions());
  } catch (e) {
    print("Invalid server config: $e");
  }
  showServerSettingsWithValue(
      ServerConfig.fromOptions(options), dialogManager, setState);
}

void showServerSettingsWithValue(
    ServerConfig serverConfig,
    OverlayDialogManager dialogManager,
    void Function(VoidCallback)? upSetState) async {
  const labelWidth = 120.0;
  const labelSpacing = 8.0;
  var isInProgress = false;
  final idCtrl = TextEditingController(text: serverConfig.idServer);
  final relayCtrl = TextEditingController(text: serverConfig.relayServer);
  final apiCtrl = TextEditingController(text: serverConfig.apiServer);
  final keyCtrl = TextEditingController(text: serverConfig.key);

  RxString idServerMsg = ''.obs;
  RxString relayServerMsg = ''.obs;
  RxString apiServerMsg = ''.obs;

  final controllers = [idCtrl, relayCtrl, apiCtrl, keyCtrl];
  final errMsgs = [
    idServerMsg,
    relayServerMsg,
    apiServerMsg,
  ];

  final profiles = _loadServerProfiles();
  // -1: Default (empty config), null: unsaved custom config
  int? selectedProfile = _matchServerProfile(profiles, serverConfig);

  dialogManager.show((setState, close, context) {
    void selectProfile(int? i) {
      if (i == null) return;
      final sc = i < 0 ? ServerConfig() : profiles[i].$2;
      idCtrl.text = sc.idServer;
      // relay field is hidden there, same as importConfig
      relayCtrl.text = isWeb || isIOS ? '' : sc.relayServer;
      apiCtrl.text = sc.apiServer;
      keyCtrl.text = sc.key;
      for (final msg in errMsgs) {
        msg.value = '';
      }
      setState(() => selectedProfile = i);
    }

    Future<void> saveProfile() async {
      final name = await _inputServerProfileName(dialogManager,
          selectedProfile != null && selectedProfile! >= 0
              ? profiles[selectedProfile!].$1
              : '');
      if (name == null || name.isEmpty) return;
      // same normalization as setServerConfig, so the profile matches once applied
      String strip(String s) {
        s = s.trim();
        return s.endsWith('/') ? s.substring(0, s.length - 1) : s;
      }

      final sc = ServerConfig(
          idServer: strip(idCtrl.text),
          relayServer: strip(relayCtrl.text),
          apiServer: strip(apiCtrl.text),
          key: keyCtrl.text);
      var i = profiles.indexWhere((p) => p.$1 == name);
      if (i < 0) {
        profiles.add((name, sc));
        i = profiles.length - 1;
      } else {
        profiles[i] = (name, sc);
      }
      await _saveServerProfiles(profiles);
      setState(() => selectedProfile = i);
      _showSuccess();
    }

    Future<void> deleteProfile() async {
      final i = selectedProfile;
      if (i == null || i < 0) return;
      profiles.removeAt(i);
      setState(() => selectedProfile = null);
      await _saveServerProfiles(profiles);
    }

    Widget buildProfileRow() {
      final row = Row(
        children: [
          Expanded(
            child: DropdownButton<int>(
              isExpanded: true,
              value: selectedProfile,
              hint: Text('-'),
              items: [
                DropdownMenuItem(
                    value: -1, child: Text(translate('Default'))),
                for (var i = 0; i < profiles.length; i++)
                  DropdownMenuItem(
                      value: i,
                      child: Text(profiles[i].$1,
                          overflow: TextOverflow.ellipsis)),
              ],
              onChanged: selectProfile,
            ),
          ),
          Tooltip(
            message: translate('Save'),
            child: IconButton(
                icon: Icon(Icons.bookmark_add_outlined, color: Colors.grey),
                onPressed: saveProfile),
          ),
          Tooltip(
            message: translate('Delete'),
            child: IconButton(
                color: Colors.grey,
                icon: Icon(Icons.delete_outline),
                onPressed: selectedProfile != null && selectedProfile! >= 0
                    ? deleteProfile
                    : null),
          ),
        ],
      );
      if (isDesktop || isWeb) {
        return Row(
          children: [
            SizedBox(width: labelWidth, child: Text(translate('Alias'))),
            SizedBox(width: labelSpacing),
            Expanded(child: row),
          ],
        );
      }
      return row;
    }

    Future<bool> submit() async {
      setState(() {
        isInProgress = true;
      });
      bool ret = await setServerConfig(
          null,
          errMsgs,
          ServerConfig(
              idServer: idCtrl.text.trim(),
              relayServer: relayCtrl.text.trim(),
              apiServer: apiCtrl.text.trim(),
              key: keyCtrl.text.trim()));
      setState(() {
        isInProgress = false;
      });
      return ret;
    }

    Widget buildField(
        String label, TextEditingController controller, String errorMsg,
        {String? Function(String?)? validator, bool autofocus = false}) {
      if (isDesktop || isWeb) {
        return Row(
          children: [
            SizedBox(
              width: labelWidth,
              child: Text(label),
            ),
            SizedBox(width: labelSpacing),
            Expanded(
              child: serverSettingsTextFormField(
                label: label,
                controller: controller,
                errorMsg: errorMsg,
                contentPadding:
                    EdgeInsets.symmetric(horizontal: 8, vertical: 12),
                showLabelText: false,
                validator: validator,
                autofocus: autofocus,
              ).workaroundFreezeLinuxMint(),
            ),
          ],
        );
      }

      return serverSettingsTextFormField(
        label: label,
        controller: controller,
        errorMsg: errorMsg,
        validator: validator,
      ).workaroundFreezeLinuxMint();
    }

    return CustomAlertDialog(
      title: Row(
        children: [
          Expanded(child: Text(translate('ID/Relay Server'))),
          ...ServerConfigImportExportWidgets(controllers, errMsgs),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Form(
          child: Obx(() => Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  buildProfileRow(),
                  SizedBox(height: 8),
                  buildField(translate('ID Server'), idCtrl, idServerMsg.value,
                      autofocus: true),
                  SizedBox(height: 8),
                  if (!isIOS && !isWeb) ...[
                    buildField(translate('Relay Server'), relayCtrl,
                        relayServerMsg.value),
                    SizedBox(height: 8),
                  ],
                  buildField(
                    translate('API Server'),
                    apiCtrl,
                    apiServerMsg.value,
                    validator: (v) {
                      if (v != null && v.isNotEmpty) {
                        if (!(v.startsWith('http://') ||
                            v.startsWith("https://"))) {
                          return translate("invalid_http");
                        }
                      }
                      return null;
                    },
                  ),
                  Padding(
                    padding: EdgeInsetsDirectional.only(
                      start: isDesktop || isWeb ? labelWidth + labelSpacing : 0,
                    ),
                    child: ApiServerWarning(
                      controller: apiCtrl,
                      idController: idCtrl,
                      message: translate('api_server_connection_tip'),
                      emptyMessage: translate('api_server_empty_tip'),
                    ),
                  ),
                  SizedBox(height: 8),
                  buildField('Key', keyCtrl, ''),
                  if (isInProgress)
                    Padding(
                      padding: EdgeInsets.only(top: 8),
                      child: LinearProgressIndicator(),
                    ),
                ],
              )),
        ),
      ),
      actions: [
        dialogButton('Cancel', onPressed: () {
          close();
        }, isOutline: true),
        dialogButton(
          'OK',
          onPressed: () async {
            if (await submit()) {
              close();
              showToast(translate('Successful'));
              upSetState?.call(() {});
            } else {
              showToast(translate('Failed'));
            }
          },
        ),
      ],
    );
  });
}

const _kServerProfilesKey = 'server-profiles';

List<(String, ServerConfig)> _loadServerProfiles() {
  final raw = bind.mainGetLocalOption(key: _kServerProfilesKey);
  if (raw.isEmpty) return [];
  try {
    return (jsonDecode(raw) as List)
        .map((e) => (e['name'] as String, ServerConfig.decode(jsonEncode(e))))
        .toList();
  } catch (e) {
    debugPrint("Invalid server profiles: $e");
    return [];
  }
}

Future<void> _saveServerProfiles(List<(String, ServerConfig)> profiles) {
  final list = profiles
      .map((p) => {
            'name': p.$1,
            'host': p.$2.idServer,
            'relay': p.$2.relayServer,
            'api': p.$2.apiServer,
            'key': p.$2.key,
          })
      .toList();
  return bind.mainSetLocalOption(
      key: _kServerProfilesKey, value: jsonEncode(list));
}

int? _matchServerProfile(
    List<(String, ServerConfig)> profiles, ServerConfig sc) {
  bool eq(ServerConfig a) =>
      a.idServer == sc.idServer &&
      a.relayServer == sc.relayServer &&
      a.apiServer == sc.apiServer &&
      a.key == sc.key;
  if (eq(ServerConfig())) return -1;
  final i = profiles.indexWhere((p) => eq(p.$2));
  return i < 0 ? null : i;
}

Future<String?> _inputServerProfileName(
    OverlayDialogManager dialogManager, String initial) {
  final ctrl = TextEditingController(text: initial);
  return dialogManager.show<String?>((setState, close, context) {
    submit() => close(ctrl.text.trim());
    return CustomAlertDialog(
      title: Text(translate('Alias')),
      content: TextField(
        controller: ctrl,
        autofocus: true,
      ),
      actions: [
        dialogButton('Cancel', onPressed: () => close(null), isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: () => close(null),
    );
  });
}

TextFormField serverSettingsTextFormField({
  required String label,
  required TextEditingController controller,
  required String errorMsg,
  String? Function(String?)? validator,
  bool autofocus = false,
  bool showLabelText = true,
  EdgeInsetsGeometry? contentPadding,
}) {
  return TextFormField(
    controller: controller,
    decoration: InputDecoration(
      labelText: showLabelText ? label : null,
      errorText: errorMsg.isEmpty ? null : errorMsg,
      contentPadding: contentPadding,
    ),
    validator: validator,
    autofocus: autofocus,
    keyboardType: TextInputType.visiblePassword,
    textCapitalization: TextCapitalization.none,
    autocorrect: false,
    enableSuggestions: false,
    smartDashesType: SmartDashesType.disabled,
    smartQuotesType: SmartQuotesType.disabled,
    enableIMEPersonalizedLearning: false,
    spellCheckConfiguration: const SpellCheckConfiguration.disabled(),
  );
}

void setPrivacyModeDialog(
  OverlayDialogManager dialogManager,
  List<TToggleMenu> privacyModeList,
  RxString privacyModeState,
) async {
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate('Privacy mode')),
      content: Column(
          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
          children: privacyModeList
              .map((value) => CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    visualDensity: VisualDensity.compact,
                    title: value.child,
                    value: value.value,
                    onChanged: value.onChanged,
                  ))
              .toList()),
    );
  }, backDismiss: true, clickMaskDismiss: true);
}
