import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:async';
import 'dart:convert';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:typed_data';

class RgbaFrame extends Struct {
  @Uint32()
  int len;
  Pointer<Uint8> data;
}

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
typedef F4 = void Function(Pointer<RgbaFrame>);
typedef F5 = Pointer<RgbaFrame> Function();

// https://juejin.im/post/6844903864852807694
class FfiModel with ChangeNotifier {
  FfiModel() {
    init();
  }

  Future<Null> init() async {
    await FFI.init();
    notifyListeners();
  }
}

class FFI {
  static F1 _freeCString;
  static F2 _getByName;
  static F3 _setByName;
  static F4 _freeRgba;
  static F5 _getRgba;
  static Pointer<RgbaFrame> _lastRgbaFrame;

  static String getId() {
    return getByName('remote_id');
  }

  static List<Peer> peers() {
    try {
      List<dynamic> peers = json.decode(getByName('peers'));
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

  static void connect(String id) {
    setByName('connect', id);
  }

  static void _clearRgbaFrame() {
    if (_lastRgbaFrame != null && _lastRgbaFrame != nullptr)
      _freeRgba(_lastRgbaFrame);
  }

  static Uint8List getRgba() {
    _clearRgbaFrame();
    _lastRgbaFrame = _getRgba();
    if (_lastRgbaFrame == null || _lastRgbaFrame == nullptr) return null;
    final ref = _lastRgbaFrame.ref;
    return Uint8List.sublistView(ref.data.asTypedList(ref.len));
  }

  static Map<String, String> popEvent() {
    var s = getByName('event');
    if (s == '') return null;
    try {
      Map<String, String> event = json.decode(s);
      return event;
    } catch (e) {
      print(e);
    }
    return null;
  }

  static void login(String password, bool remember) {
    setByName(
        'login',
        json.encode({
          'password': password,
          'remember': remember ? 'true' : 'false',
        }));
  }

  static void close() {
    _clearRgbaFrame();
    setByName('close', '');
  }

  static void setByName(String name, String value) {
    _setByName(Utf8.toUtf8(name), Utf8.toUtf8(value));
  }

  static String getByName(String name, {String arg = ''}) {
    var p = _getByName(Utf8.toUtf8(name), Utf8.toUtf8(arg));
    assert(p != nullptr && p != null);
    var res = Utf8.fromUtf8(p);
    // https://github.com/brickpop/flutter-rust-ffi
    _freeCString(p);
    return res;
  }

  static Future<Null> init() async {
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : DynamicLibrary.process();
    _getByName = dylib.lookupFunction<F2, F2>('get_by_name');
    _setByName =
        dylib.lookupFunction<Void Function(Pointer<Utf8>, Pointer<Utf8>), F3>(
            'set_by_name');
    _freeCString = dylib
        .lookupFunction<Void Function(Pointer<Utf8>), F1>('rust_cstr_free');
    _freeRgba = dylib
        .lookupFunction<Void Function(Pointer<RgbaFrame>), F4>('free_rgba');
    _getRgba = dylib.lookupFunction<F5, F5>('get_rgba');
    final dir = (await getApplicationDocumentsDirectory()).path;
    setByName('init', dir);
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

// https://material.io/develop/flutter/components/dialogs
void enterPasswordDialog(String id, BuildContext context) {
  var remember = FFI.getByName('remember', arg: id) == 'true';
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
