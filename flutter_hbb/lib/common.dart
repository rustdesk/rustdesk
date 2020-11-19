import 'package:flutter/material.dart';
import 'dart:async';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:tuple/tuple.dart';

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
  static const Color canvasColor = Color(0xFF212121);
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
Future<Null> showAlertDialog(
    BuildContext context,
    Tuple3<Widget, Widget, List<Widget>> Function(
            void Function(void Function()))
        build) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  var dialog = StatefulBuilder(builder: (context, setState) {
    var widgets = build(setState);
    return AlertDialog(
      title: widgets.item1,
      contentPadding: const EdgeInsets.all(20.0),
      content: widgets.item2,
      actions: widgets.item3,
    );
  });
  await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => dialog);
  _hasDialog = false;
}

void msgbox(String type, String title, String text, BuildContext context) {
  showAlertDialog(
      context,
      (_) => Tuple3(Text(title), Text(text), [
            FlatButton(
              textColor: MyTheme.accent,
              onPressed: () {
                Navigator.pop(context);
                Navigator.pop(context);
              },
              child: Text('OK'),
            ),
          ]));
}
