import 'package:flutter/material.dart';
import 'dart:async';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'model.dart';

class HexColor extends Color {
  HexColor(final String hexColor) : super(_getColorFromHex(hexColor));

  static int _getColorFromHex(String hexColor) {
    hexColor = hexColor.toUpperCase().replaceAll('#', '');
    if (hexColor.length == 6) {
      hexColor = 'FF' + hexColor;
    }
    return int.parse(hexColor, radix: 16);
  }
}

class MyTheme {
  static const Color grayBg = Color(0xFFEEEEEE);
  static const Color white = Color(0xFFFFFFFF);
  static const Color accent = Color(0xFF0071FF);
  static const Color accent50 = Color(0x770071FF);
}

// https://github.com/huangjianke/flutter_easyloading
void showLoading(String text) {
  dismissLoading();
  EasyLoading.show(status: text);
}

void dismissLoading() {
  EasyLoading.dismiss();
}

void showSuccess(String text) {
  dismissLoading();
  EasyLoading.showSuccess(text);
}

bool _hasDialog = false;

// https://material.io/develop/flutter/components/dialogs
Future<Null> enterPasswordDialog(String id, BuildContext context) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  final controller = TextEditingController();
  var remember = FFI.getByName('remember', arg: id) == 'true';
  var dialog = StatefulBuilder(builder: (context, setState) {
    return AlertDialog(
      title: Text('Please enter your password'),
      contentPadding: const EdgeInsets.all(20.0),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            autofocus: true,
            obscureText: true,
            controller: controller,
            decoration: const InputDecoration(
              labelText: 'Password',
            ),
          ),
          ListTile(
            title: Text(
              'Remember the password',
            ),
            leading: Checkbox(
              value: remember,
              onChanged: (v) {
                setState(() {
                  remember = v;
                });
              },
            ),
          ),
        ],
      ),
      actions: [
        FlatButton(
          textColor: MyTheme.accent,
          onPressed: () {
            Navigator.pop(context);
            Navigator.pop(context);
          },
          child: Text('Cancel'),
        ),
        FlatButton(
          textColor: MyTheme.accent,
          onPressed: () {
            var text = controller.text.trim();
            if (text == '') return;
            FFI.login(text, remember);
            showLoading('Logging in...');
            Navigator.pop(context);
          },
          child: Text('OK'),
        ),
      ],
    );
  });
  await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => dialog);
  _hasDialog = false;
}

Future<Null> wrongPasswordDialog(String id, BuildContext context) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  var dialog = AlertDialog(
    title: Text('Wrong Password'),
    contentPadding: const EdgeInsets.all(20.0),
    content: Text('Do you want to enter again?'),
    actions: [
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () {
          Navigator.pop(context);
          Navigator.pop(context);
        },
        child: Text('Cancel'),
      ),
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () {
          Navigator.pop(context);
          enterPasswordDialog(id, context);
        },
        child: Text('Retry'),
      ),
    ],
  );
  await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => dialog);
  _hasDialog = false;
}

Future<Null> msgbox(
    String type, String title, String text, BuildContext context) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  var dialog = AlertDialog(
    title: Text(title),
    contentPadding: const EdgeInsets.all(20.0),
    content: Text(text),
    actions: [
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () {
          Navigator.pop(context);
          Navigator.pop(context);
        },
        child: Text('OK'),
      ),
    ],
  );
  await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => dialog);
  _hasDialog = false;
}
