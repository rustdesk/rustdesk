import 'package:flutter/material.dart';
import 'package:tuple/tuple.dart';
import '../common.dart';
import '../models/model.dart';

void clientClose() {
  msgBox('', 'Close', 'Are you sure to close the connection?');
}

void enterPasswordDialog(String id) {
  final controller = TextEditingController();
  var remember = FFI.getByName('remember', id) == 'true';
  if (globalKey.currentContext == null) return;
  showAlertDialog((setState) => Tuple3(
    Text(translate('Password Required')),
    Column(mainAxisSize: MainAxisSize.min, children: [
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
    [
      TextButton(
        style: flatButtonStyle,
        onPressed: () {
          DialogManager.reset();
          Navigator.pop(globalKey.currentContext!);
        },
        child: Text(translate('Cancel')),
      ),
      TextButton(
        style: flatButtonStyle,
        onPressed: () {
          var text = controller.text.trim();
          if (text == '') return;
          FFI.login(text, remember);
          DialogManager.reset();
          showLoading(translate('Logging in...'));
        },
        child: Text(translate('OK')),
      ),
    ],
  ));
}

void wrongPasswordDialog(String id) {
  if (globalKey.currentContext == null) return;
  showAlertDialog((_) => Tuple3(Text(translate('Wrong Password')),
      Text(translate('Do you want to enter again?')), [
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            DialogManager.reset();
            Navigator.pop(globalKey.currentContext!);
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