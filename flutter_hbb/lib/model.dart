import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:math';
import 'dart:ffi';
import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:tuple/tuple.dart';
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

class FfiModel with ChangeNotifier {
  PeerInfo _pi;
  Display _display;
  bool _decoding;
  bool _waitForImage;
  final _permissions = Map<String, bool>();

  get permissions => _permissions;

  FfiModel() {
    clear();
    () async {
      await FFI.init();
      notifyListeners();
    }();
  }

  void updatePermission(Map<String, dynamic> evt) {
    evt.forEach((k, v) {
      if (k == 'name') return;
      _permissions[k] = v == 'true';
    });
    print('$_permissions');
  }

  bool keyboard() => _permissions['keyboard'] == true;

  void clear() {
    _pi = PeerInfo();
    _display = Display();
    _decoding = false;
    _waitForImage = false;
    _permissions.clear();
  }

  void update(
      String id,
      BuildContext context,
      void Function(Map<String, dynamic> evt, String id, BuildContext context)
          handleMsgbox) {
    var pos;
    for (;;) {
      var evt = FFI.popEvent();
      if (evt == null) break;
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
        pos = evt;
      } else if (name == 'permission') {
        FFI.ffiModel.updatePermission(evt);
      }
    }
    if (pos != null) FFI.cursorModel.updateCursorPosition(pos);
    if (!_decoding) {
      var rgba = FFI.getRgba();
      if (rgba != null) {
        if (_waitForImage) {
          _waitForImage = false;
          final size = MediaQueryData.fromWindow(ui.window).size;
          final xscale = size.width / _display.width.toDouble();
          final yscale = size.height / _display.height.toDouble();
          FFI.canvasModel.scale = max(xscale, yscale);
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
    if (displays.length > 0) {
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
    if (image != null) notifyListeners();
  }

  double get maxScale {
    if (_image == null) return 1.0;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image.width;
    final yscale = size.height / _image.height;
    return max(1.0, max(xscale, yscale));
  }

  double get minScale {
    if (_image == null) return 1.0;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image.width;
    final yscale = size.height / _image.height;
    return min(xscale, yscale);
  }
}

class CanvasModel with ChangeNotifier {
  double _x;
  double _y;
  double _scale;
  double _xPan;
  double _yPan;

  CanvasModel() {
    clear();
  }

  double get x => _x;
  double get y => _y;
  double get scale => _scale;

  set scale(v) {
    _scale = v;
    notifyListeners();
  }

  void startPan() {
    _xPan = 0;
    _yPan = 0;
  }

  void updateOffset(double dx, double dy) {
    _x += dx;
    if (_x > 0) {
      _x = 0;
      dx = -dx;
    }
    _y += dy;
    if (_y > 0) {
      _y = 0;
      dy = -dy;
    }
    _xPan += dx;
    _yPan += dy;
    var px = (_xPan > 0 ? _xPan.floor() : _xPan.ceil()).toDouble();
    var py = (_yPan > 0 ? _yPan.floor() : _yPan.ceil()).toDouble();
    if (px != 0 || py != 0) {
      FFI.cursorModel.update(-px, -py);
      _xPan -= px;
      _yPan -= py;
    }
    notifyListeners();
  }

  void updateScale(double v) {
    _scale *= v;
    final maxs = FFI.imageModel.maxScale;
    final mins = FFI.imageModel.minScale;
    if (_scale > maxs) _scale = maxs;
    if (_scale < mins) _scale = mins;
    notifyListeners();
  }

  void clear() {
    _x = 0;
    _y = 0;
    _scale = 1.0;
    _xPan = 0;
    _yPan = 0;
  }
}

class CursorModel with ChangeNotifier {
  ui.Image _image;
  final _images = Map<int, Tuple3<ui.Image, double, double>>();
  double _x = -10000;
  double _y = -10000;
  double _hotx = 0;
  double _hoty = 0;
  double _displayOriginX = 0;
  double _displayOriginY = 0;

  ui.Image get image => _image;
  double get x => _x - _displayOriginX;
  double get y => _y - _displayOriginY;
  double get hotx => _hotx;
  double get hoty => _hoty;

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
      _images[id] = Tuple3(image, _hotx, _hoty);
      try {
        // my throw exception, because the listener maybe already dispose
        notifyListeners();
      } catch (e) {}
    });
  }

  void updateCursorId(Map<String, dynamic> evt) {
    final tmp = _images[int.parse(evt['id'])];
    if (tmp != null) {
      _image = tmp.item1;
      _hotx = tmp.item2;
      _hoty = tmp.item3;
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

  void update(double dx, double dy) {
    _x += dx;
    _y += dy;
    var x = _x.toInt();
    var y = _y.toInt();
    FFI.setByName('send_mouse', json.encode({'x': '$x', 'y': '$y'}));
  }

  void clear() {
    _x = -10000;
    _x = -10000;
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
  static final canvasModel = CanvasModel();

  static String getId() {
    return getByName('remote_id');
  }

  static void tap() {
    FFI.sendMouse('down', 'left');
    FFI.sendMouse('up', 'left');
  }

  static void sendMouse(String type, String buttons) {
    FFI.setByName(
        'send_mouse', json.encode({'type': type, 'buttons': buttons}));
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
    FFI.canvasModel.clear();
  }

  static void setByName(String name, [String value = '']) {
    _setByName(Utf8.toUtf8(name), Utf8.toUtf8(value));
  }

  static String getByName(String name, [String arg = '']) {
    var p = _getByName(Utf8.toUtf8(name), Utf8.toUtf8(arg));
    assert(p != nullptr && p != null);
    var res = Utf8.fromUtf8(p);
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
