import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:async';
import 'dart:convert';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:typed_data';
import 'dart:ui' as ui;

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

class ImageModel with ChangeNotifier {
  ui.Image _image;

  ui.Image get image => _image;

  void update(ui.Image image) {
    _image = image;
    notifyListeners();
  }
}

class CursorModel with ChangeNotifier {
  ui.Image _image;
  final _images = Map<int, ui.Image>();
  double _x = 0;
  double _y = 0;
  double _hotx = 0;
  double _hoty = 0;

  ui.Image get image => _image;
  double get x => _x;
  double get y => _y;

  void updateCursorData(Map<String, dynamic> evt) {
    var id = int.parse(evt['id']);
    _hotx = double.parse(evt['hotx']);
    _hoty = double.parse(evt['hoty']);
    var width = int.parse(evt['width']);
    var height = int.parse(evt['height']);
    List<dynamic> colors = json.decode(evt['colors']);
    final rgba = Uint8List.fromList(colors.map((s) => s as int).toList());
    ui.decodeImageFromPixels(rgba, width, height, ui.PixelFormat.rgba8888,
        (image) {
      _image = image;
      _images[id] = image;
      notifyListeners();
    });
  }

  void updateCursorId(Map<String, dynamic> evt) {
    final tmp = _images[int.parse(evt['id'])];
    if (tmp != null) {
      _image = tmp;
      notifyListeners();
    }
  }

  void updateCursorPosition(Map<String, dynamic> evt) {
    _x = double.parse(evt['x']);
    _y = double.parse(evt['y']);
    notifyListeners();
  }

  void clear() {
    _image = null;
    _images.clear();
  }
}

class FFI {
  static F1 _freeCString;
  static F2 _getByName;
  static F3 _setByName;
  static F4 _freeRgba;
  static F5 _getRgba;
  static Pointer<RgbaFrame> _lastRgbaFrame;
  static final imageModel = ImageModel();
  static final ffiModel = FfiModel();
  static final cursorModel = CursorModel();

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

  static void clearRgbaFrame() {
    if (_lastRgbaFrame != null && _lastRgbaFrame != nullptr)
      _freeRgba(_lastRgbaFrame);
  }

  static Uint8List getRgba() {
    _lastRgbaFrame = _getRgba();
    if (_lastRgbaFrame == null || _lastRgbaFrame == nullptr) return null;
    final ref = _lastRgbaFrame.ref;
    return Uint8List.sublistView(ref.data.asTypedList(ref.len));
  }

  static Map<String, dynamic> popEvent() {
    var s = getByName('event');
    if (s == '') return null;
    try {
      Map<String, dynamic> event = json.decode(s);
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
    setByName('close', '');
    FFI.imageModel.update(null);
    FFI.cursorModel.clear();
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
