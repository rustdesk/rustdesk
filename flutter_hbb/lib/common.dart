import 'package:flutter/material.dart';
import 'dart:async';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:tuple/tuple.dart';

typedef F = String Function(String);

class Translator {
  static F call;
}

class MyTheme {
  MyTheme._();
  static const Color grayBg = Color(0xFFEEEEEE);
  static const Color white = Color(0xFFFFFFFF);
  static const Color accent = Color(0xFF0071FF);
  static const Color accent50 = Color(0x770071FF);
  static const Color accent80 = Color(0xAA0071FF);
  static const Color canvasColor = Color(0xFF212121);
  static const Color border = Color(0xFFCCCCCC);
}

final ButtonStyle flatButtonStyle = TextButton.styleFrom(
  minimumSize: Size(88, 36),
  padding: EdgeInsets.symmetric(horizontal: 16.0),
  shape: const RoundedRectangleBorder(
    borderRadius: BorderRadius.all(Radius.circular(2.0)),
  ),
);

void showLoading(String text, BuildContext context) {
  if (_hasDialog && context != null) {
    Navigator.pop(context);
  }
  dismissLoading();
  EasyLoading.show(status: text, maskType: EasyLoadingMaskType.black);
}

void dismissLoading() {
  EasyLoading.dismiss();
}

void showSuccess(String text) {
  dismissLoading();
  EasyLoading.showSuccess(text, maskType: EasyLoadingMaskType.black);
}

bool _hasDialog = false;
typedef BuildAlertDailog = Tuple3<Widget, Widget, List<Widget>> Function(
    void Function(void Function()));

Future<T> showAlertDialog<T>(BuildContext context, BuildAlertDailog build,
    [WillPopCallback onWillPop,
    bool barrierDismissible = false,
    double contentPadding = 20]) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  var dialog = StatefulBuilder(builder: (context, setState) {
    var widgets = build(setState);
    if (onWillPop == null) onWillPop = () async => false;
    return WillPopScope(
        onWillPop: onWillPop,
        child: AlertDialog(
          title: widgets.item1,
          contentPadding: EdgeInsets.all(contentPadding),
          content: widgets.item2,
          actions: widgets.item3,
        ));
  });
  var res = await showDialog<T>(
      context: context,
      barrierDismissible: barrierDismissible,
      builder: (context) => dialog);
  _hasDialog = false;
  return res;
}

void msgbox(String type, String title, String text, BuildContext context,
    [bool hasCancel]) {
  var wrap = (String text, void Function() onPressed) => ButtonTheme(
      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      materialTapTargetSize: MaterialTapTargetSize
          .shrinkWrap, //limits the touch area to the button area
      minWidth: 0, //wraps child's width
      height: 0,
      child: TextButton(
          style: flatButtonStyle,
          onPressed: onPressed,
          child: Text(Translator.call(text),
              style: TextStyle(color: MyTheme.accent))));

  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  final buttons = [
    Expanded(child: Container()),
    wrap(Translator.call('OK'), () {
      dismissLoading();
      Navigator.pop(context);
    })
  ];
  if (hasCancel == null) {
    hasCancel = type != 'error';
  }
  if (hasCancel) {
    buttons.insert(
        1,
        wrap(Translator.call('Cancel'), () {
          dismissLoading();
        }));
  }
  EasyLoading.showWidget(
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(Translator.call(title), style: TextStyle(fontSize: 21)),
          SizedBox(height: 20),
          Text(Translator.call(text), style: TextStyle(fontSize: 15)),
          SizedBox(height: 20),
          Row(
            children: buttons,
          )
        ],
      ),
      maskType: EasyLoadingMaskType.black);
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({Key key, this.controller}) : super(key: key);

  final TextEditingController controller;

  @override
  _PasswordWidgetState createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  @override
  Widget build(BuildContext context) {
    return TextField(
      autofocus: true,
      controller: widget.controller,
      obscureText: !_passwordVisible, //This will obscure text dynamically
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

Color str2color(String str, [alpha = 0xFF]) {
  var hash = 160 << 16 + 114 << 8 + 91;
  for (var i = 0; i < str.length; i += 1) {
    hash = str.codeUnitAt(i) + ((hash << 5) - hash);
  }
  hash = hash % 16777216;
  return Color((hash & 0xFF7FFF) | (alpha << 24));
}
