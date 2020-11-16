import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:async';
import 'dart:convert';
import 'package:flutter_easyloading/flutter_easyloading.dart';

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
}

typedef F1 = void Function(Pointer<Utf8>);
typedef F2 = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F3 = void Function(Pointer<Utf8>, Pointer<Utf8>);

// https://juejin.im/post/6844903864852807694
class FfiModel with ChangeNotifier {
  F1 _freeCString;
  F2 _getByName;
  F3 _setByName;

  FfiModel() {
    initialzeFFI();
  }

  void addRemote() {
    notifyListeners();
  }

  void connect(String id) {
    setByName("connect", id);
    _setByName(Utf8.toUtf8("connect"), Utf8.toUtf8(id));
  }

  void setByName(String name, String value) {
    _setByName(Utf8.toUtf8(name), Utf8.toUtf8(value));
  }

  String getByName(String name, {String arg = ""}) {
    var p = _getByName(Utf8.toUtf8(name), Utf8.toUtf8(arg));
    var res = Utf8.fromUtf8(p);
    // https://github.com/brickpop/flutter-rust-ffi
    _freeCString(p);
    return res;
  }

  String getId() {
    return getByName("remote_id");
  }

  List<Peer> peers() {
    try {
      List<dynamic> peers = json.decode(getByName("peers"));
      return peers
          .map((s) => s as List<dynamic>)
          .map((s) =>
              Peer.fromJson(s[0] as String, s[1] as Map<String, dynamic>))
          .toList();
    } catch (e) {
      print(e);
    }
    return [];
  }

  Future<Null> initialzeFFI() async {
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : DynamicLibrary.process();
    _getByName = dylib.lookupFunction<F2, F2>('get_by_name');
    _setByName =
        dylib.lookupFunction<Void Function(Pointer<Utf8>, Pointer<Utf8>), F3>(
            'set_by_name');
    _freeCString = dylib
        .lookupFunction<Void Function(Pointer<Utf8>), F1>('rust_cstr_free');
    final dir = (await getApplicationDocumentsDirectory()).path;
    setByName("init", dir);
    notifyListeners();
  }
}

class Peer {
  final String id;
  final String username;
  final String hostname;
  final String platform;

  Peer.fromJson(String id, Map<String, dynamic> json)
      : id = id,
        username = json['username'],
        hostname = json['hostname'],
        platform = json['platform'];
}

// https://github.com/huangjianke/flutter_easyloading
void showLoading(String text) {
  EasyLoading.show(status: text);
}

void dismissLoading() {
  EasyLoading.dismiss();
}

void showSuccess(String text) {
  EasyLoading.showSuccess(text);
}

void enterPasswordDialog(String id, BuildContext context) {
  var ffi = Provider.of<FfiModel>(context);
  var remember = ffi.getByName("remember", arg: id) == "true";
  var dialog = AlertDialog(
    title: Text('Please enter your password'),
    contentPadding: EdgeInsets.zero,
    content: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        TextField(
          obscureText: true,
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
            onChanged: (_) {},
          ),
        ),
      ],
    ),
    actions: [
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () => Navigator.pop(context),
        child: Text('Cancel'),
      ),
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () => Navigator.pop(context),
        child: Text('OK'),
      ),
    ],
  );
  showDialog<void>(context: context, builder: (context) => dialog);
}

void wrongPasswordDialog(String id, BuildContext context) {
  var dialog = AlertDialog(
    title: Text('Please enter your password'),
    contentPadding: EdgeInsets.zero,
    content: Text('Do you want to enter again?'),
    actions: [
      FlatButton(
        textColor: MyTheme.accent,
        onPressed: () => Navigator.pop(context),
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
  showDialog<void>(context: context, builder: (context) => dialog);
}
