import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

void _showSuccess() {
  showToast(translate("Successful"));
}

void _showError() {
  showToast(translate("Error"));
}

void setPermanentPasswordDialog(OverlayDialogManager dialogManager) async {
  final pw = await bind.mainGetPermanentPassword();
  final p0 = TextEditingController(text: pw);
  final p1 = TextEditingController(text: pw);
  var validateLength = false;
  var validateSame = false;
  dialogManager.show((setState, close, context) {
    submit() async {
      close();
      dialogManager.showLoading(translate("Waiting"));
      if (await gFFI.serverModel.setPermanentPassword(p0.text)) {
        dialogManager.dismissAll();
        _showSuccess();
      } else {
        dialogManager.dismissAll();
        _showError();
      }
    }

    return CustomAlertDialog(
      title: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('Set your own password')).paddingOnly(left: 10),
        ],
      ),
      content: Form(
          autovalidateMode: AutovalidateMode.onUserInteraction,
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            TextFormField(
              autofocus: true,
              obscureText: true,
              keyboardType: TextInputType.visiblePassword,
              decoration: InputDecoration(
                labelText: translate('Password'),
              ),
              controller: p0,
              validator: (v) {
                if (v == null) return null;
                final val = v.trim().length > 5;
                if (validateLength != val) {
                  // use delay to make setState success
                  Future.delayed(Duration(microseconds: 1),
                      () => setState(() => validateLength = val));
                }
                return val
                    ? null
                    : translate('Too short, at least 6 characters.');
              },
            ),
            TextFormField(
              obscureText: true,
              keyboardType: TextInputType.visiblePassword,
              decoration: InputDecoration(
                labelText: translate('Confirmation'),
              ),
              controller: p1,
              validator: (v) {
                if (v == null) return null;
                final val = p0.text == v;
                if (validateSame != val) {
                  Future.delayed(Duration(microseconds: 1),
                      () => setState(() => validateSame = val));
                }
                return val
                    ? null
                    : translate('The confirmation is not identical.');
              },
            ),
          ])),
      onCancel: close,
      onSubmit: (validateLength && validateSame) ? submit : null,
      actions: [
        dialogButton(
          'Cancel',
          icon: Icon(Icons.close_rounded),
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          icon: Icon(Icons.done_rounded),
          onPressed: (validateLength && validateSame) ? submit : null,
        ),
      ],
    );
  });
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

void showServerSettingsWithValue(
    ServerConfig serverConfig, OverlayDialogManager dialogManager) async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  final oldCfg = ServerConfig.fromOptions(oldOptions);

  var isInProgress = false;
  final idCtrl = TextEditingController(text: serverConfig.idServer);
  final relayCtrl = TextEditingController(text: serverConfig.relayServer);
  final apiCtrl = TextEditingController(text: serverConfig.apiServer);
  final keyCtrl = TextEditingController(text: serverConfig.key);

  String? idServerMsg;
  String? relayServerMsg;
  String? apiServerMsg;

  dialogManager.show((setState, close, context) {
    Future<bool> validate() async {
      if (idCtrl.text != oldCfg.idServer) {
        final res = await validateAsync(idCtrl.text);
        setState(() => idServerMsg = res);
        if (idServerMsg != null) return false;
      }
      if (relayCtrl.text != oldCfg.relayServer) {
        relayServerMsg = await validateAsync(relayCtrl.text);
        if (relayServerMsg != null) return false;
      }
      if (apiCtrl.text != oldCfg.apiServer) {
        if (apiServerMsg != null) return false;
      }
      return true;
    }

    return CustomAlertDialog(
      title: Text(translate('ID/Relay Server')),
      content: Form(
          child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                    TextFormField(
                      controller: idCtrl,
                      decoration: InputDecoration(
                          labelText: translate('ID Server'),
                          errorText: idServerMsg),
                    )
                  ] +
                  (isAndroid
                      ? [
                          TextFormField(
                            controller: relayCtrl,
                            decoration: InputDecoration(
                                labelText: translate('Relay Server'),
                                errorText: relayServerMsg),
                          )
                        ]
                      : []) +
                  [
                    TextFormField(
                      controller: apiCtrl,
                      decoration: InputDecoration(
                        labelText: translate('API Server'),
                      ),
                      autovalidateMode: AutovalidateMode.onUserInteraction,
                      validator: (v) {
                        if (v != null && v.isNotEmpty) {
                          if (!(v.startsWith('http://') ||
                              v.startsWith("https://"))) {
                            return translate("invalid_http");
                          }
                        }
                        return apiServerMsg;
                      },
                    ),
                    TextFormField(
                      controller: keyCtrl,
                      decoration: InputDecoration(
                        labelText: 'Key',
                      ),
                    ),
                    Offstage(
                        offstage: !isInProgress,
                        child: LinearProgressIndicator())
                  ])),
      actions: [
        dialogButton('Cancel', onPressed: () {
          close();
        }, isOutline: true),
        dialogButton(
          'OK',
          onPressed: () async {
            setState(() {
              idServerMsg = null;
              relayServerMsg = null;
              apiServerMsg = null;
              isInProgress = true;
            });
            if (await validate()) {
              if (idCtrl.text != oldCfg.idServer) {
                if (oldCfg.idServer.isNotEmpty) {
                  await gFFI.userModel.logOut();
                }
                bind.mainSetOption(
                    key: "custom-rendezvous-server", value: idCtrl.text);
              }
              if (relayCtrl.text != oldCfg.relayServer) {
                bind.mainSetOption(key: "relay-server", value: relayCtrl.text);
              }
              if (keyCtrl.text != oldCfg.key) {
                bind.mainSetOption(key: "key", value: keyCtrl.text);
              }
              if (apiCtrl.text != oldCfg.apiServer) {
                bind.mainSetOption(key: "api-server", value: apiCtrl.text);
              }
              close();
              showToast(translate('Successful'));
            }
            setState(() {
              isInProgress = false;
            });
          },
        ),
      ],
    );
  });
}

Future<String?> validateAsync(String value) async {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = await bind.mainTestIfValidServer(server: value);
  return res.isEmpty ? null : res;
}
