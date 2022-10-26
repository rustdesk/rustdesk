import 'dart:async';
import 'package:flutter/material.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';

void clientClose(OverlayDialogManager dialogManager) {
  msgBox(
      '', 'Close', 'Are you sure to close the connection?', '', dialogManager);
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
              Icon(Icons.warning_amber_sharp,
                  color: Colors.redAccent, size: 28),
              SizedBox(width: 10),
              Text(translate("Restart Remote Device")),
            ]),
            content: Text(
                "${translate('Are you sure you want to restart')} \n${pi.username}@${pi.hostname}($id) ?"),
            actions: [
              TextButton(
                  onPressed: () => close(), child: Text(translate("Cancel"))),
              ElevatedButton(
                  onPressed: () => close(true), child: Text(translate("OK"))),
            ],
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
      actions: [
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            close();
          },
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: (validateLength && validateSame)
              ? () async {
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
              : null,
          child: Text(translate('OK')),
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
        TextButton(
          style: flatButtonStyle,
          onPressed: cancel,
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: submit,
          child: Text(translate('OK')),
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void wrongPasswordDialog(String id, OverlayDialogManager dialogManager) {
  dialogManager.show((setState, close) => CustomAlertDialog(
          title: Text(translate('Wrong Password')),
          content: Text(translate('Do you want to enter again?')),
          actions: [
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                close();
                closeConnection();
              },
              child: Text(translate('Cancel')),
            ),
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                enterPasswordDialog(id, dialogManager);
              },
              child: Text(translate('Retry')),
            ),
          ]));
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
            color: Theme.of(context).primaryColorDark,
          ),
          onPressed: () {
            // Update the state i.e. toogle the state of passwordVisible variable
            setState(() {
              _passwordVisible = !_passwordVisible;
            });
          },
        ),
      ),
    );
  }
}
