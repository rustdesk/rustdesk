import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import '../common.dart';
import '../models/model.dart';

void clientClose() {
  msgBox('', 'Close', 'Are you sure to close the connection?');
}

const SEC1 = Duration(seconds: 1);
void showSuccess({Duration duration = SEC1}) {
  SmartDialog.dismiss();
  showToast(translate("Successful"), duration: SEC1);
}

void showError({Duration duration = SEC1}) {
  SmartDialog.dismiss();
  showToast(translate("Error"), duration: SEC1);
}

void setPermanentPasswordDialog() {
  final pw = FFI.getByName("permanent_password");
  final p0 = TextEditingController(text: pw);
  final p1 = TextEditingController(text: pw);
  var validateLength = false;
  var validateSame = false;
  DialogManager.show((setState, close) {
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
                  showLoading(translate("Waiting"));
                  if (await FFI.serverModel.setPermanentPassword(p0.text)) {
                    showSuccess();
                  } else {
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

void setTemporaryPasswordLengthDialog() {
  List<String> lengths = ['6', '8', '10'];
  String length = FFI.getByName('option', 'temporary-password-length');
  var index = lengths.indexOf(length);
  if (index < 0) index = 0;
  length = lengths[index];
  DialogManager.show((setState, close) {
    final setLength = (newValue) {
      final oldValue = length;
      if (oldValue == newValue) return;
      setState(() {
        length = newValue;
      });
      Map<String, String> msg = Map()
        ..["name"] = "temporary-password-length"
        ..["value"] = newValue;
      FFI.setByName("option", jsonEncode(msg));
      FFI.setByName("temporary_password");
      Future.delayed(Duration(milliseconds: 200), () {
        close();
        showSuccess();
      });
    };
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

void enterPasswordDialog(String id) {
  final controller = TextEditingController();
  var remember = FFI.getByName('remember', id) == 'true';
  DialogManager.show((setState, close) {
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
          onPressed: () {
            close();
            backToHome();
          },
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            var text = controller.text.trim();
            if (text == '') return;
            FFI.login(text, remember);
            close();
            showLoading(translate('Logging in...'));
          },
          child: Text(translate('OK')),
        ),
      ],
    );
  });
}

void wrongPasswordDialog(String id) {
  DialogManager.show((setState, close) => CustomAlertDialog(
          title: Text(translate('Wrong Password')),
          content: Text(translate('Do you want to enter again?')),
          actions: [
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                close();
                backToHome();
              },
              child: Text(translate('Cancel')),
            ),
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                enterPasswordDialog(id);
              },
              child: Text(translate('Retry')),
            ),
          ]));
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({Key? key, required this.controller}) : super(key: key);

  final TextEditingController controller;

  @override
  _PasswordWidgetState createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  final _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    Timer(Duration(milliseconds: 50), () => _focusNode.requestFocus());
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
        labelText: Translator.call('Password'),
        hintText: Translator.call('Enter your password'),
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
