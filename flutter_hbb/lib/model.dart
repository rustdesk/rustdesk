import 'package:ffi/ffi.dart';
import 'package:flutter/gestures.dart';
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
  bool _initialized = false;
  final _permissions = Map<String, bool>();

  get permissions => _permissions;
  get initialized => _initialized;

  FfiModel() {
    clear();
    () async {
      await FFI.init();
      _initialized = true;
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

  bool keyboard() => _permissions['keyboard'] != false;

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
    if (_image == null && image != null) {
      final size = MediaQueryData.fromWindow(ui.window).size;
      final xscale = size.width / image.width;
      final yscale = size.height / image.height;
      FFI.canvasModel.scale = max(xscale, yscale);
    }
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

  void panX(double dx) {
    _x += dx;
    notifyListeners();
  }

  void panY(double dy) {
    _y += dy;
    notifyListeners();
  }

  void updateScale(double v) {
    final offset = FFI.cursorModel.offset;
    var r = FFI.cursorModel.getVisibleRect();
    final px0 = (offset.dx - r.left) * _scale;
    final py0 = (offset.dy - r.top) * _scale;
    _scale *= v;
    final maxs = FFI.imageModel.maxScale;
    final mins = FFI.imageModel.minScale;
    if (_scale > maxs) _scale = maxs;
    if (_scale < mins) _scale = mins;
    r = FFI.cursorModel.getVisibleRect();
    final px1 = (offset.dx - r.left) * _scale;
    final py1 = (offset.dy - r.top) * _scale;
    _x -= px1 - px0;
    _y -= py1 - py0;
    notifyListeners();
  }

  void clear() {
    _x = 0;
    _y = 0;
    _scale = 1.0;
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
  Offset get offset => Offset(_x, _y);
  double get hotx => _hotx;
  double get hoty => _hoty;

  // remote physical display coordinate
  Rect getVisibleRect() {
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xoffset = FFI.canvasModel.x;
    final yoffset = FFI.canvasModel.y;
    final scale = FFI.canvasModel.scale;
    final x0 = _displayOriginX - xoffset / scale;
    final y0 = _displayOriginY - yoffset / scale;
    return Rect.fromLTWH(x0, y0, size.width / scale, size.height / scale);
  }

  double adjustForKeyboard() {
    var keyboardHeight = MediaQueryData.fromWindow(ui.window).viewInsets.bottom;
    if (keyboardHeight < 100) return 0;
    final s = FFI.canvasModel.scale;
    final thresh = 120;
    var h = (_y - getVisibleRect().top) * s; // local physical display height
    return h > thresh ? h - thresh : 0;
  }

  void updatePan(double dx, double dy) {
    final scale = FFI.canvasModel.scale;
    dx /= scale;
    dy /= scale;
    final r = getVisibleRect();
    var cx = r.center.dx;
    var cy = r.center.dy;
    var tryMoveCanvasX = false;
    if (dx > 0) {
      final maxCanvasCanMove =
          _displayOriginX + FFI.imageModel.image.width - r.right;
      tryMoveCanvasX = _x + dx > cx && maxCanvasCanMove > 0;
      if (tryMoveCanvasX) {
        dx = min(dx, maxCanvasCanMove);
      } else {
        final maxCursorCanMove = r.right - _x;
        dx = min(dx, maxCursorCanMove);
      }
    } else if (dx < 0) {
      final maxCanvasCanMove = _displayOriginX - r.left;
      tryMoveCanvasX = _x + dx < cx && maxCanvasCanMove < 0;
      if (tryMoveCanvasX) {
        dx = max(dx, maxCanvasCanMove);
      } else {
        final maxCursorCanMove = r.left - _x;
        dx = max(dx, maxCursorCanMove);
      }
    }
    var tryMoveCanvasY = false;
    if (dy > 0) {
      final mayCanvasCanMove =
          _displayOriginY + FFI.imageModel.image.height - r.bottom;
      tryMoveCanvasY = _y + dy > cy && mayCanvasCanMove > 0;
      if (tryMoveCanvasY) {
        dy = min(dy, mayCanvasCanMove);
      } else {
        final mayCursorCanMove = r.bottom - _y;
        dy = min(dy, mayCursorCanMove);
      }
    } else if (dy < 0) {
      final mayCanvasCanMove = _displayOriginY - r.top;
      tryMoveCanvasY = _y + dy < cy && mayCanvasCanMove < 0;
      if (tryMoveCanvasY) {
        dy = max(dy, mayCanvasCanMove);
      } else {
        final mayCursorCanMove = r.top - _y;
        dy = max(dy, mayCursorCanMove);
      }
    }

    if (dx == 0 && dy == 0) return;
    _x += dx;
    _y += dy;
    if (tryMoveCanvasX && dx != 0) {
      FFI.canvasModel.panX(-dx);
    }
    if (tryMoveCanvasY && dy != 0) {
      FFI.canvasModel.panY(-dy);
    }

    FFI.moveMouse(_x, _y);
    notifyListeners();
  }

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
    _x = x;
    _y = y;
    FFI.moveMouse(x, y);
    notifyListeners();
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
  static var shift = false;
  static var ctrl = false;
  static var alt = false;
  static var command = false;
  static final imageModel = ImageModel();
  static final ffiModel = FfiModel();
  static final cursorModel = CursorModel();
  static final canvasModel = CanvasModel();

  static String getId() {
    return getByName('remote_id');
  }

  static void tap(bool right) {
    sendMouse('down', right ? 'right' : 'left');
    sendMouse('up', right ? 'right' : 'left');
  }

  static void scroll(double y) {
    var y2 = y.round();
    if (y2 == 0) return;
    setByName('send_mouse',
        json.encode(modify({'type': 'wheel', 'y': y2.toString()})));
  }

  static void resetModifiers() {
    shift = ctrl = alt = command = false;
  }

  static Map<String, String> modify(Map<String, String> evt) {
    if (ctrl) evt['ctrl'] = 'true';
    if (shift) evt['shift'] = 'true';
    if (alt) evt['alt'] = 'true';
    if (command) evt['command'] = 'true';
    return evt;
  }

  static void sendMouse(String type, String buttons) {
    if (!ffiModel.keyboard()) return;
    setByName(
        'send_mouse', json.encode(modify({'type': type, 'buttons': buttons})));
  }

  static void inputKey(String name) {
    if (!ffiModel.keyboard()) return;
    setByName('input_key', json.encode(modify({'name': name})));
  }

  static void moveMouse(double x, double y) {
    if (!ffiModel.keyboard()) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    setByName('send_mouse', json.encode(modify({'x': '$x2', 'y': '$y2'})));
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
    imageModel.update(null);
    cursorModel.clear();
    ffiModel.clear();
    canvasModel.clear();
    resetModifiers();
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
