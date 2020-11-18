import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'dart:async';
import 'common.dart';

class RgbaFrame extends Struct {
  @Uint32()
  int len;
  Pointer<Uint8> data;
}

typedef F1 = void Function(Pointer<Utf8>);
typedef F2 = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F3 = void Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F4 = void Function(Pointer<RgbaFrame>);
typedef F5 = Pointer<RgbaFrame> Function();

// https://juejin.im/post/6844903864852807694
class FfiModel with ChangeNotifier {
  PeerInfo _pi = PeerInfo();
  Display _display = Display();
  bool _decoding = false;
  bool _waitForImage = false;

  FfiModel() {
    init();
  }

  Future<Null> init() async {
    await FFI.init();
    notifyListeners();
  }

  void clear() {
    _decoding = false;
  }

  void update(String id, BuildContext context) {
    var evt = FFI.popEvent();
    if (evt != null) {
      var name = evt['name'];
      if (name == 'msgbox') {
        handleMsgbox(evt, id, context);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt);
      } else if (name == 'switch_display') {
        handleSwitchDisplay(evt);
      } else if (name == 'cursor_data') {
        FFI.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        FFI.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        FFI.cursorModel.updateCursorPosition(evt);
      }
    }
    if (!_decoding) {
      var rgba = FFI.getRgba();
      if (rgba != null) {
        if (_waitForImage) {
          _waitForImage = false;
          dismissLoading();
        }
        _decoding = true;
        ui.decodeImageFromPixels(
            rgba, _display.width, _display.height, ui.PixelFormat.bgra8888,
            (image) {
          FFI.clearRgbaFrame();
          _decoding = false;
          try {
            // my throw exception, because the listener maybe already dispose
            FFI.imageModel.update(image);
          } catch (e) {}
        });
      }
    }
  }

  void handleMsgbox(Map<String, dynamic> evt, String id, BuildContext context) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id, context);
    } else if (type == 'input-password') {
      enterPasswordDialog(id, context);
    } else {
      msgbox(type, title, text, context);
    }
  }

  void handleSwitchDisplay(Map<String, dynamic> evt) {
    _pi.currentDisplay = int.parse(evt['display']);
    _display.x = double.parse(evt['x']);
    _display.y = double.parse(evt['y']);
    _display.width = int.parse(evt['width']);
    _display.height = int.parse(evt['height']);
    FFI.cursorModel.updateDisplayOrigin(_display.x, _display.y);
  }

  void handlePeerInfo(Map<String, dynamic> evt) {
    dismissLoading();
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == "true";
    _pi.currentDisplay = int.parse(evt['current_display']);
    List<dynamic> displays = json.decode(evt['displays']);
    _pi.displays = List<Display>();
    for (int i = 0; i < displays.length; ++i) {
      Map<String, dynamic> d0 = displays[i];
      var d = Display();
      d.x = d0['x'].toDouble();
      d.y = d0['y'].toDouble();
      d.width = d0['width'];
      d.height = d0['height'];
      _pi.displays.add(d);
    }
    if (_pi.currentDisplay < _pi.displays.length) {
      _display = _pi.displays[_pi.currentDisplay];
      FFI.cursorModel.updateDisplayOrigin(_display.x, _display.y);
    }
    if (displays.length > 1) {
      showLoading('Waiting for image...');
      _waitForImage = true;
    }
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
  double _displayOriginX = 0;
  double _displayOriginY = 0;

  ui.Image get image => _image;
  double get x => _x - _displayOriginX - _hotx;
  double get y => _y - _displayOriginY - _hoty;

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
      try {
        // my throw exception, because the listener maybe already dispose
        notifyListeners();
      } catch (e) {}
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

  void updateDisplayOrigin(double x, double y) {
    _displayOriginX = x;
    _displayOriginY = y;
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
    FFI.ffiModel.clear();
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

class Display {
  double x = 0;
  double y = 0;
  int width = 0;
  int height = 0;
}

class PeerInfo {
  String username;
  String hostname;
  String platform;
  bool sasEnabled;
  int currentDisplay;
  List<Display> displays;
}
