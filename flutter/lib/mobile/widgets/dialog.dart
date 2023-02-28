import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';

void clientClose(String id, OverlayDialogManager dialogManager) {
  msgBox(id, 'info', 'Close', 'Are you sure to close the connection?', '',
      dialogManager);
}

void showSuccess() {
  showToast(translate("Successful"));
}

void showError() {
  showToast(translate("Error"));
}

void showRestartRemoteDevice(
    PeerInfo pi, String id, OverlayDialogManager dialogManager) async {
  final res =
      await dialogManager.show<bool>((setState, close) => CustomAlertDialog(
            title: Row(children: [
              Icon(Icons.warning_rounded, color: Colors.redAccent, size: 28),
              Text(translate("Restart Remote Device")).paddingOnly(left: 10),
            ]),
            content: Column(
              children: [
                Text(
                    "${translate('Are you sure you want to restart')} \n${pi.username}@${pi.hostname}($id) ?"),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    ElevatedButton.icon(
                      icon: Icon(Icons.close_rounded),
                      label: Text(translate("Cancel")),
                      onPressed: close,
                    ),
                    ElevatedButton.icon(
                      icon: Icon(Icons.done_rounded),
                      label: Text(translate("Ok")),
                      onPressed: () => close(true),
                    ),
                  ],
                ).paddingOnly(top: 20)
              ],
            ),
            onCancel: close,
            onSubmit: () => close(true),
          ));
  if (res == true) bind.sessionRestartRemoteDevice(id: id);
}

void setPermanentPasswordDialog(OverlayDialogManager dialogManager) async {
  final pw = await bind.mainGetPermanentPassword();
  final p0 = TextEditingController(text: pw);
  final p1 = TextEditingController(text: pw);
  var validateLength = false;
  var validateSame = false;
  dialogManager.show((setState, close) {
    submit() async {
      close();
      dialogManager.showLoading(translate("Waiting"));
      if (await gFFI.serverModel.setPermanentPassword(p0.text)) {
        dialogManager.dismissAll();
        showSuccess();
      } else {
        dialogManager.dismissAll();
        showError();
      }
    }

    return CustomAlertDialog(
      title: Text(translate('Set your own password')),
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
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          'OK',
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
  dialogManager.show((setState, close) {
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
        showSuccess();
      });
    }

    return CustomAlertDialog(
      title: Text(translate("Set temporary password length")),
      content: Column(
          mainAxisSize: MainAxisSize.min,
          children:
              lengths.map((e) => getRadio(e, e, length, setLength)).toList()),
      actions: [],
      contentPadding: 14,
    );
  }, backDismiss: true, clickMaskDismiss: true);
}

void enterPasswordDialog(String id, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  var remember = await bind.sessionGetRemember(id: id) ?? false;
  dialogManager.dismissAll();
  dialogManager.show((setState, close) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      var text = controller.text.trim();
      if (text == '') return;
      gFFI.login(id, text, remember);
      close();
      dialogManager.showLoading(translate('Logging in...'),
          onCancel: closeConnection);
    }

    return CustomAlertDialog(
      title: Text(translate('Password Required')),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        PasswordWidget(controller: controller),
        CheckboxListTile(
          contentPadding: const EdgeInsets.all(0),
          dense: true,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(
            translate('Remember password'),
          ),
          value: remember,
          onChanged: (v) {
            if (v != null) {
              setState(() => remember = v);
            }
          },
        ),
      ]),
      actions: [
        dialogButton('Cancel', onPressed: cancel, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void wrongPasswordDialog(
    String id, OverlayDialogManager dialogManager, type, title, text) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      enterPasswordDialog(id, dialogManager);
    }

    return CustomAlertDialog(
        title: null,
        content: msgboxContent(type, title, text),
        onSubmit: submit,
        onCancel: cancel,
        actions: [
          dialogButton(
            'Cancel',
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            'Retry',
            onPressed: submit,
          ),
        ]);
  });
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

  dialogManager.show((setState, close) {
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

void showWaitUacDialog(
    String id, OverlayDialogManager dialogManager, String type) {
  dialogManager.dismissAll();
  dialogManager.show(
      tag: '$id-wait-uac',
      (setState, close) => CustomAlertDialog(
            title: null,
            content: msgboxContent(type, 'Wait', 'wait_accept_uac_tip'),
          ));
}

void showRequestElevationDialog(String id, OverlayDialogManager dialogManager) {
  RxString groupValue = ''.obs;
  RxString errUser = ''.obs;
  RxString errPwd = ''.obs;
  TextEditingController userController = TextEditingController();
  TextEditingController pwdController = TextEditingController();

  void onRadioChanged(String? value) {
    if (value != null) {
      groupValue.value = value;
    }
  }

  const minTextStyle = TextStyle(fontSize: 14);

  var content = Obx(() => Column(children: [
        Row(
          children: [
            Radio(
                value: '',
                groupValue: groupValue.value,
                onChanged: onRadioChanged),
            Expanded(
                child:
                    Text(translate('Ask the remote user for authentication'))),
          ],
        ),
        Align(
          alignment: Alignment.centerLeft,
          child: Text(
                  translate(
                      'Choose this if the remote account is administrator'),
                  style: TextStyle(fontSize: 13))
              .marginOnly(left: 40),
        ).marginOnly(bottom: 15),
        Row(
          children: [
            Radio(
                value: 'logon',
                groupValue: groupValue.value,
                onChanged: onRadioChanged),
            Expanded(
              child: Text(translate(
                  'Transmit the username and password of administrator')),
            )
          ],
        ),
        Row(
          children: [
            Expanded(
                flex: 1,
                child: Text(
                  '${translate('Username')}:',
                  style: minTextStyle,
                ).marginOnly(right: 10)),
            Expanded(
              flex: 3,
              child: TextField(
                controller: userController,
                style: minTextStyle,
                decoration: InputDecoration(
                    isDense: true,
                    contentPadding: EdgeInsets.symmetric(vertical: 15),
                    hintText: 'eg: admin',
                    errorText: errUser.isEmpty ? null : errUser.value),
                onChanged: (s) {
                  if (s.isNotEmpty) {
                    errUser.value = '';
                  }
                },
              ),
            )
          ],
        ).marginOnly(left: 40),
        Row(
          children: [
            Expanded(
                flex: 1,
                child: Text(
                  '${translate('Password')}:',
                  style: minTextStyle,
                ).marginOnly(right: 10)),
            Expanded(
              flex: 3,
              child: TextField(
                controller: pwdController,
                obscureText: true,
                style: minTextStyle,
                decoration: InputDecoration(
                    isDense: true,
                    contentPadding: EdgeInsets.symmetric(vertical: 15),
                    errorText: errPwd.isEmpty ? null : errPwd.value),
                onChanged: (s) {
                  if (s.isNotEmpty) {
                    errPwd.value = '';
                  }
                },
              ),
            ),
          ],
        ).marginOnly(left: 40),
        Align(
            alignment: Alignment.centerLeft,
            child: Text(translate('still_click_uac_tip'),
                    style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold))
                .marginOnly(top: 20)),
      ]));

  dialogManager.dismissAll();
  dialogManager.show(tag: '$id-request-elevation', (setState, close) {
    void submit() {
      if (groupValue.value == 'logon') {
        if (userController.text.isEmpty) {
          errUser.value = translate('Empty Username');
          return;
        }
        if (pwdController.text.isEmpty) {
          errPwd.value = translate('Empty Password');
          return;
        }
        bind.sessionElevateWithLogon(
            id: id,
            username: userController.text,
            password: pwdController.text);
      } else {
        bind.sessionElevateDirect(id: id);
      }
    }

    return CustomAlertDialog(
      title: Text(translate('Request Elevation')),
      content: content,
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showOnBlockDialog(
  String id,
  String type,
  String title,
  String text,
  OverlayDialogManager dialogManager,
) {
  if (dialogManager.existing('$id-wait-uac') ||
      dialogManager.existing('$id-request-elevation')) {
    return;
  }
  dialogManager.show(tag: '$id-$type', (setState, close) {
    void submit() {
      close();
      showRequestElevationDialog(id, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title,
          "${translate(text)}${type.contains('uac') ? '\n' : '\n\n'}${translate('request_elevation_tip')}"),
      actions: [
        dialogButton('Wait', onPressed: close, isOutline: true),
        dialogButton('Request Elevation', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showElevationError(String id, String type, String title, String text,
    OverlayDialogManager dialogManager) {
  dialogManager.show(tag: '$id-$type', (setState, close) {
    void submit() {
      close();
      showRequestElevationDialog(id, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        dialogButton('Cancel', onPressed: () {
          close();
        }, isOutline: true),
        dialogButton('Retry', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showWaitAcceptDialog(String id, String type, String title, String text,
    OverlayDialogManager dialogManager) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close) {
    onCancel() {
      closeConnection();
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        dialogButton('Cancel', onPressed: onCancel, isOutline: true),
      ],
      onCancel: onCancel,
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

class PasswordWidget extends StatefulWidget {
  PasswordWidget({Key? key, required this.controller, this.autoFocus = true})
      : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;

  @override
  State<PasswordWidget> createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  final _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    if (widget.autoFocus) {
      Timer(Duration(milliseconds: 50), () => _focusNode.requestFocus());
    }
  }

  @override
  void dispose() {
    _focusNode.unfocus();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TextField(
      focusNode: _focusNode,
      controller: widget.controller,
      obscureText: !_passwordVisible,
      //This will obscure text dynamically
      keyboardType: TextInputType.visiblePassword,
      decoration: InputDecoration(
        labelText: translate('Password'),
        hintText: translate('Enter your password'),
        // Here is key idea
        suffixIcon: IconButton(
          icon: Icon(
              // Based on passwordVisible state choose the icon
              _passwordVisible ? Icons.visibility : Icons.visibility_off,
              color: MyTheme.lightTheme.primaryColor),
          onPressed: () {
            // Update the state i.e. toggle the state of passwordVisible variable
            setState(() {
              _passwordVisible = !_passwordVisible;
            });
          },
        ),
      ),
    );
  }
}
