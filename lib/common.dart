import 'package:flutter/material.dart';
import 'dart:async';
import 'package:flutter_easyloading/flutter_easyloading.dart';

final globalKey = GlobalKey<NavigatorState>();

typedef F = String Function(String);
typedef FMethod = String Function(String, dynamic);

class Translator {
  static late F call;
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
  static const Color idColor = Color(0xFF00B6F0);
  static const Color darkGray = Color(0xFFB9BABC);
}

final ButtonStyle flatButtonStyle = TextButton.styleFrom(
  minimumSize: Size(88, 36),
  padding: EdgeInsets.symmetric(horizontal: 16.0),
  shape: const RoundedRectangleBorder(
    borderRadius: BorderRadius.all(Radius.circular(2.0)),
  ),
);

void showLoading(String text) {
  DialogManager.reset();
  EasyLoading.dismiss();
  EasyLoading.show(status: text, maskType: EasyLoadingMaskType.black);
}

backToHome() {
  // use [popUntil()] to make sure pop action can't close the current MaterialApp context
  Navigator.popUntil(globalKey.currentContext!, ModalRoute.withName("/"));
}

typedef DialogBuilder = CustomAlertDialog Function(
    StateSetter setState, VoidCallback close);

class DialogManager {
  static BuildContext? _dialogContext;

  static void reset() {
    if (_dialogContext != null) {
      Navigator.pop(_dialogContext!);
    }
    _dialogContext = null;
  }

  static void register(BuildContext dialogContext) {
    _dialogContext = dialogContext;
  }

  static void drop() {
    _dialogContext = null;
  }

  static Future<T?> show<T>(DialogBuilder builder,
      {bool barrierDismissible = false}) async {
    if (globalKey.currentContext == null) return null;
    EasyLoading.dismiss();
    DialogManager.reset();
    final res = await showDialog<T>(
        context: globalKey.currentContext!,
        barrierDismissible: barrierDismissible,
        builder: (context) {
          DialogManager.register(context);
          return StatefulBuilder(
              builder: (_, setState) => builder(setState, DialogManager.reset));
        });
    DialogManager.drop();
    return res;
  }
}

class CustomAlertDialog extends StatelessWidget {
  CustomAlertDialog(
      {required this.title,
      required this.content,
      required this.actions,
      this.onWillPop,
      this.contentPadding});

  final Widget title;
  final Widget content;
  final List<Widget> actions;
  final WillPopCallback? onWillPop;
  final double? contentPadding;

  @override
  Widget build(BuildContext context) {
    return WillPopScope(
        onWillPop: onWillPop ?? () async => false,
        child: AlertDialog(
          title: title,
          contentPadding: EdgeInsets.all(contentPadding ?? 20),
          content: content,
          actions: actions,
        ));
  }
}

// EasyLoading
void msgBox(String type, String title, String text, {bool? hasCancel}) {
  var wrap = (String text, void Function() onPressed) => ButtonTheme(
      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      //limits the touch area to the button area
      minWidth: 0,
      //wraps child's width
      height: 0,
      child: TextButton(
          style: flatButtonStyle,
          onPressed: onPressed,
          child: Text(Translator.call(text),
              style: TextStyle(color: MyTheme.accent))));

  EasyLoading.dismiss();
  DialogManager.reset();
  final buttons = [
    Expanded(child: Container()),
    wrap(Translator.call('OK'), () {
      EasyLoading.dismiss();
      backToHome();
    })
  ];
  if (hasCancel == null) {
    hasCancel = type != 'error';
  }
  if (hasCancel) {
    buttons.insert(
        1,
        wrap(Translator.call('Cancel'), () {
          EasyLoading.dismiss();
        }));
  }
  EasyLoading.show(
      status: "",
      maskType: EasyLoadingMaskType.black,
      indicator: Container(
          constraints: BoxConstraints(maxWidth: 300),
          child: Column(
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
          )));
}

Color str2color(String str, [alpha = 0xFF]) {
  var hash = 160 << 16 + 114 << 8 + 91;
  for (var i = 0; i < str.length; i += 1) {
    hash = str.codeUnitAt(i) + ((hash << 5) - hash);
  }
  hash = hash % 16777216;
  return Color((hash & 0xFF7FFF) | (alpha << 24));
}

bool isAndroid = false;
bool isIOS = false;
bool isWeb = false;
bool isDesktop = false;
var version = "";
