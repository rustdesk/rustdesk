import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'dart:math';
import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:tuple/tuple.dart';
import 'dart:async';
import '../common.dart';
import '../widgets/dialog.dart';
import '../widgets/overlay.dart';
import 'native_model.dart' if (dart.library.html) 'web_model.dart';

typedef HandleMsgBox = void Function(Map<String, dynamic> evt, String id);
bool _waitForImage = false;

class FfiModel with ChangeNotifier {
  PeerInfo _pi = PeerInfo();
  Display _display = Display();
  var _inputBlocked = false;
  final _permissions = Map<String, bool>();
  bool? _secure;
  bool? _direct;
  bool _touchMode = false;
  Timer? _timer;
  var _reconnects = 1;

  Map<String, bool> get permissions => _permissions;

  Display get display => _display;

  bool? get secure => _secure;

  bool? get direct => _direct;

  PeerInfo get pi => _pi;

  bool get inputBlocked => _inputBlocked;

  bool get touchMode => _touchMode;

  bool get isPeerAndroid => _pi.platform == "Android";

  set inputBlocked(v) {
    _inputBlocked = v;
  }

  FfiModel() {
    Translator.call = translate;
    clear();
  }

  Future<void> init() async {
    await PlatformFFI.init();
  }

  void toggleTouchMode() {
    if (!isPeerAndroid) {
      _touchMode = !_touchMode;
      notifyListeners();
    }
  }

  void updatePermission(Map<String, dynamic> evt) {
    evt.forEach((k, v) {
      if (k == 'name' || k.isEmpty) return;
      _permissions[k] = v == 'true';
    });
    print('$_permissions');
    notifyListeners();
  }

  void updateUser() {
    notifyListeners();
  }

  bool keyboard() => _permissions['keyboard'] != false;

  void clear() {
    _pi = PeerInfo();
    _display = Display();
    _waitForImage = false;
    _secure = null;
    _direct = null;
    _inputBlocked = false;
    _timer?.cancel();
    _timer = null;
    clearPermissions();
  }

  void setConnectionType(bool secure, bool direct) {
    _secure = secure;
    _direct = direct;
  }

  Image? getConnectionImage() {
    String? icon;
    if (secure == true && direct == true) {
      icon = 'secure';
    } else if (secure == false && direct == true) {
      icon = 'insecure';
    } else if (secure == false && direct == false) {
      icon = 'insecure_relay';
    } else if (secure == true && direct == false) {
      icon = 'secure_relay';
    }
    return icon == null
        ? null
        : Image.asset('assets/$icon.png', width: 48, height: 48);
  }

  void clearPermissions() {
    _inputBlocked = false;
    _permissions.clear();
  }

  void updateEventListener(String peerId) {
    final void Function(Map<String, dynamic>) cb = (evt) {
      var name = evt['name'];
      if (name == 'msgbox') {
        handleMsgBox(evt, peerId);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt);
      } else if (name == 'connection_ready') {
        FFI.ffiModel.setConnectionType(
            evt['secure'] == 'true', evt['direct'] == 'true');
      } else if (name == 'switch_display') {
        handleSwitchDisplay(evt);
      } else if (name == 'cursor_data') {
        FFI.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        FFI.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        FFI.cursorModel.updateCursorPosition(evt);
      } else if (name == 'clipboard') {
        Clipboard.setData(ClipboardData(text: evt['content']));
      } else if (name == 'permission') {
        FFI.ffiModel.updatePermission(evt);
      } else if (name == 'chat_client_mode') {
        FFI.chatModel.receive(ChatModel.clientModeID, evt['text'] ?? "");
      } else if (name == 'chat_server_mode') {
        FFI.chatModel
            .receive(int.parse(evt['id'] as String), evt['text'] ?? "");
      } else if (name == 'file_dir') {
        FFI.fileModel.receiveFileDir(evt);
      } else if (name == 'job_progress') {
        FFI.fileModel.tryUpdateJobProgress(evt);
      } else if (name == 'job_done') {
        FFI.fileModel.jobDone(evt);
      } else if (name == 'job_error') {
        FFI.fileModel.jobError(evt);
      } else if (name == 'override_file_confirm') {
        FFI.fileModel.overrideFileConfirm(evt);
      } else if (name == 'try_start_without_auth') {
        FFI.serverModel.loginRequest(evt);
      } else if (name == 'on_client_authorized') {
        FFI.serverModel.onClientAuthorized(evt);
      } else if (name == 'on_client_remove') {
        FFI.serverModel.onClientRemove(evt);
      } else if (name == 'update_quality_status') {
        FFI.qualityMonitorModel.updateQualityStatus(evt);
      }
    };
    PlatformFFI.setEventCallback(cb);
  }

  void handleSwitchDisplay(Map<String, dynamic> evt) {
    final oldOrientation = _display.width > _display.height;
    var old = _pi.currentDisplay;
    _pi.currentDisplay = int.parse(evt['display']);
    _display.x = double.parse(evt['x']);
    _display.y = double.parse(evt['y']);
    _display.width = int.parse(evt['width']);
    _display.height = int.parse(evt['height']);
    if (old != _pi.currentDisplay)
      FFI.cursorModel.updateDisplayOrigin(_display.x, _display.y);

    // remote is mobile, and orientation changed
    if ((_display.width > _display.height) != oldOrientation) {
      FFI.canvasModel.updateViewStyle();
    }
    notifyListeners();
  }

  void handleMsgBox(Map<String, dynamic> evt, String id) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id);
    } else if (type == 'input-password') {
      enterPasswordDialog(id);
    } else if (type == 'restarting') {
      showMsgBox(type, title, text, false, hasCancel: false);
    } else {
      var hasRetry = evt['hasRetry'] == 'true';
      showMsgBox(type, title, text, hasRetry);
    }
  }

  void showMsgBox(String type, String title, String text, bool hasRetry,
      {bool? hasCancel}) {
    msgBox(type, title, text, hasCancel: hasCancel);
    _timer?.cancel();
    if (hasRetry) {
      _timer = Timer(Duration(seconds: _reconnects), () {
        FFI.reconnect();
        showLoading(translate('Connecting...'));
      });
      _reconnects *= 2;
    } else {
      _reconnects = 1;
    }
  }

  void handlePeerInfo(Map<String, dynamic> evt) {
    SmartDialog.dismiss();
    _pi.version = evt['version'];
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == "true";
    _pi.currentDisplay = int.parse(evt['current_display']);

    if (isPeerAndroid) {
      _touchMode = true;
      if (FFI.ffiModel.permissions['keyboard'] != false) {
        Timer(Duration(milliseconds: 100), showMobileActionsOverlay);
      }
    } else {
      _touchMode = FFI.getByName('peer_option', "touch-mode") != '';
    }

    if (evt['is_file_transfer'] == "true") {
      FFI.fileModel.onReady();
    } else {
      _pi.displays = [];
      List<dynamic> displays = json.decode(evt['displays']);
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
      }
      if (displays.length > 0) {
        showLoading(translate('Connected, waiting for image...'));
        _waitForImage = true;
        _reconnects = 1;
      }
    }
    notifyListeners();
  }
}

class ImageModel with ChangeNotifier {
  ui.Image? _image;

  ui.Image? get image => _image;

  ImageModel() {
    PlatformFFI.setRgbaCallback((rgba) {
      if (_waitForImage) {
        _waitForImage = false;
        SmartDialog.dismiss();
      }
      final pid = FFI.id;
      ui.decodeImageFromPixels(
          rgba,
          FFI.ffiModel.display.width,
          FFI.ffiModel.display.height,
          isWeb ? ui.PixelFormat.rgba8888 : ui.PixelFormat.bgra8888, (image) {
        if (FFI.id != pid) return;
        try {
          // my throw exception, because the listener maybe already dispose
          FFI.imageModel.update(image);
        } catch (e) {
          print('update image: $e');
        }
      });
    });
  }

  void update(ui.Image? image) {
    if (_image == null && image != null) {
      if (isDesktop) {
        FFI.canvasModel.updateViewStyle();
      } else {
        final size = MediaQueryData.fromWindow(ui.window).size;
        final xscale = size.width / image.width;
        final yscale = size.height / image.height;
        FFI.canvasModel.scale = max(xscale, yscale);
      }
      initializeCursorAndCanvas();
      Future.delayed(Duration(milliseconds: 1), () {
        if (FFI.ffiModel.isPeerAndroid) {
          FFI.setByName(
              'peer_option', '{"name": "view-style", "value": "shrink"}');
          FFI.canvasModel.updateViewStyle();
        }
      });
    }
    _image = image;
    if (image != null) notifyListeners();
  }

  double get maxScale {
    if (_image == null) return 1.5;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return max(1.5, max(xscale, yscale));
  }

  double get minScale {
    if (_image == null) return 1.5;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return min(xscale, yscale) / 1.5;
  }
}

class CanvasModel with ChangeNotifier {
  double _x = 0;
  double _y = 0;
  double _scale = 1.0;

  CanvasModel();

  double get x => _x;

  double get y => _y;

  double get scale => _scale;

  void updateViewStyle() {
    final s = FFI.getByName('peer_option', 'view-style');
    final size = MediaQueryData.fromWindow(ui.window).size;
    final s1 = size.width / FFI.ffiModel.display.width;
    final s2 = size.height / FFI.ffiModel.display.height;
    if (s == 'shrink') {
      final s = s1 < s2 ? s1 : s2;
      if (s < 1) {
        _scale = s;
      }
    } else if (s == 'stretch') {
      final s = s1 > s2 ? s1 : s2;
      if (s > 1) {
        _scale = s;
      }
    } else {
      _scale = 1;
    }
    _x = (size.width - FFI.ffiModel.display.width * _scale) / 2;
    _y = (size.height - FFI.ffiModel.display.height * _scale) / 2;
    notifyListeners();
  }

  void update(double x, double y, double scale) {
    _x = x;
    _y = y;
    _scale = scale;
    notifyListeners();
  }

  void moveDesktopMouse(double x, double y) {
    final size = MediaQueryData.fromWindow(ui.window).size;
    final dw = FFI.ffiModel.display.width * _scale;
    final dh = FFI.ffiModel.display.height * _scale;
    var dxOffset = 0;
    var dyOffset = 0;
    if (dw > size.width) {
      dxOffset = (x - dw * (x / size.width) - _x).toInt();
    }
    if (dh > size.height) {
      dyOffset = (y - dh * (y / size.height) - _y).toInt();
    }
    _x += dxOffset;
    _y += dyOffset;
    if (dxOffset != 0 || dyOffset != 0) {
      notifyListeners();
    }
    FFI.cursorModel.moveLocal(x, y);
  }

  set scale(v) {
    _scale = v;
    notifyListeners();
  }

  void panX(double dx) {
    _x += dx;
    notifyListeners();
  }

  void resetOffset() {
    if (isDesktop) {
      updateViewStyle();
    } else {
      _x = 0;
      _y = 0;
    }
    notifyListeners();
  }

  void panY(double dy) {
    _y += dy;
    notifyListeners();
  }

  void updateScale(double v) {
    if (FFI.imageModel.image == null) return;
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

  void clear([bool notify = false]) {
    _x = 0;
    _y = 0;
    _scale = 1.0;
    if (notify) notifyListeners();
  }
}

class CursorModel with ChangeNotifier {
  ui.Image? _image;
  final _images = Map<int, Tuple3<ui.Image, double, double>>();
  double _x = -10000;
  double _y = -10000;
  double _hotx = 0;
  double _hoty = 0;
  double _displayOriginX = 0;
  double _displayOriginY = 0;

  ui.Image? get image => _image;

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
    final m = MediaQueryData.fromWindow(ui.window);
    var keyboardHeight = m.viewInsets.bottom;
    final size = m.size;
    if (keyboardHeight < 100) return 0;
    final s = FFI.canvasModel.scale;
    final thresh = (size.height - keyboardHeight) / 2;
    var h = (_y - getVisibleRect().top) * s; // local physical display height
    return h - thresh;
  }

  void touch(double x, double y, MouseButtons button) {
    moveLocal(x, y);
    FFI.moveMouse(_x, _y);
    FFI.tap(button);
  }

  void move(double x, double y) {
    moveLocal(x, y);
    FFI.moveMouse(_x, _y);
  }

  void moveLocal(double x, double y) {
    final scale = FFI.canvasModel.scale;
    final xoffset = FFI.canvasModel.x;
    final yoffset = FFI.canvasModel.y;
    _x = (x - xoffset) / scale + _displayOriginX;
    _y = (y - yoffset) / scale + _displayOriginY;
    notifyListeners();
  }

  void reset() {
    _x = _displayOriginX;
    _y = _displayOriginY;
    FFI.moveMouse(_x, _y);
    FFI.canvasModel.clear(true);
    notifyListeners();
  }

  void updatePan(double dx, double dy, bool touchMode) {
    if (FFI.imageModel.image == null) return;
    if (touchMode) {
      final scale = FFI.canvasModel.scale;
      _x += dx / scale;
      _y += dy / scale;
      FFI.moveMouse(_x, _y);
      notifyListeners();
      return;
    }
    final scale = FFI.canvasModel.scale;
    dx /= scale;
    dy /= scale;
    final r = getVisibleRect();
    var cx = r.center.dx;
    var cy = r.center.dy;
    var tryMoveCanvasX = false;
    if (dx > 0) {
      final maxCanvasCanMove = _displayOriginX +
          FFI.imageModel.image!.width -
          r.right.roundToDouble();
      tryMoveCanvasX = _x + dx > cx && maxCanvasCanMove > 0;
      if (tryMoveCanvasX) {
        dx = min(dx, maxCanvasCanMove);
      } else {
        final maxCursorCanMove = r.right - _x;
        dx = min(dx, maxCursorCanMove);
      }
    } else if (dx < 0) {
      final maxCanvasCanMove = _displayOriginX - r.left.roundToDouble();
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
      final mayCanvasCanMove = _displayOriginY +
          FFI.imageModel.image!.height -
          r.bottom.roundToDouble();
      tryMoveCanvasY = _y + dy > cy && mayCanvasCanMove > 0;
      if (tryMoveCanvasY) {
        dy = min(dy, mayCanvasCanMove);
      } else {
        final mayCursorCanMove = r.bottom - _y;
        dy = min(dy, mayCursorCanMove);
      }
    } else if (dy < 0) {
      final mayCanvasCanMove = _displayOriginY - r.top.roundToDouble();
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
    var pid = FFI.id;
    ui.decodeImageFromPixels(rgba, width, height, ui.PixelFormat.rgba8888,
        (image) {
      if (FFI.id != pid) return;
      _image = image;
      _images[id] = Tuple3(image, _hotx, _hoty);
      try {
        // my throw exception, because the listener maybe already dispose
        notifyListeners();
      } catch (e) {
        print('notify cursor: $e');
      }
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
    _x = x + 1;
    _y = y + 1;
    FFI.moveMouse(x, y);
    FFI.canvasModel.resetOffset();
    notifyListeners();
  }

  void updateDisplayOriginWithCursor(
      double x, double y, double xCursor, double yCursor) {
    _displayOriginX = x;
    _displayOriginY = y;
    _x = xCursor;
    _y = yCursor;
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

class QualityMonitorData {
  String? speed;
  String? fps;
  String? delay;
  String? targetBitrate;
  String? codecFormat;
}

class QualityMonitorModel with ChangeNotifier {
  var _show = FFI.getByName('toggle_option', 'show-quality-monitor') == 'true';
  final _data = QualityMonitorData();

  bool get show => _show;
  QualityMonitorData get data => _data;

  checkShowQualityMonitor() {
    final show =
        FFI.getByName('toggle_option', 'show-quality-monitor') == 'true';
    if (_show != show) {
      _show = show;
      notifyListeners();
    }
  }

  updateQualityStatus(Map<String, dynamic> evt) {
    try {
      if ((evt["speed"] as String).isNotEmpty) _data.speed = evt["speed"];
      if ((evt["fps"] as String).isNotEmpty) _data.fps = evt["fps"];
      if ((evt["delay"] as String).isNotEmpty) _data.delay = evt["delay"];
      if ((evt["target_bitrate"] as String).isNotEmpty)
        _data.targetBitrate = evt["target_bitrate"];
      if ((evt["codec_format"] as String).isNotEmpty)
        _data.codecFormat = evt["codec_format"];
      notifyListeners();
    } catch (e) {}
  }
}

enum MouseButtons { left, right, wheel }

extension ToString on MouseButtons {
  String get value {
    switch (this) {
      case MouseButtons.left:
        return "left";
      case MouseButtons.right:
        return "right";
      case MouseButtons.wheel:
        return "wheel";
    }
  }
}

class FFI {
  static var id = "";
  static var shift = false;
  static var ctrl = false;
  static var alt = false;
  static var command = false;
  static var version = "";
  static final imageModel = ImageModel();
  static final ffiModel = FfiModel();
  static final cursorModel = CursorModel();
  static final canvasModel = CanvasModel();
  static final serverModel = ServerModel();
  static final chatModel = ChatModel();
  static final fileModel = FileModel();
  static final qualityMonitorModel = QualityMonitorModel();

  static String getId() {
    return getByName('remote_id');
  }

  static void tap(MouseButtons button) {
    sendMouse('down', button);
    sendMouse('up', button);
  }

  static void scroll(int y) {
    setByName('send_mouse',
        json.encode(modify({'type': 'wheel', 'y': y.toString()})));
  }

  static void reconnect() {
    setByName('reconnect');
    FFI.ffiModel.clearPermissions();
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

  static void sendMouse(String type, MouseButtons button) {
    if (!ffiModel.keyboard()) return;
    setByName('send_mouse',
        json.encode(modify({'type': type, 'buttons': button.value})));
  }

  static void inputKey(String name, {bool? down, bool? press}) {
    if (!ffiModel.keyboard()) return;
    final Map<String, String> out = Map();
    out['name'] = name;
    // default: down = false
    if (down == true) {
      out['down'] = "true";
    }
    // default: press = true
    if (press != false) {
      out['press'] = "true";
    }
    setByName('input_key', json.encode(modify(out)));
  }

  static void moveMouse(double x, double y) {
    if (!ffiModel.keyboard()) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    setByName('send_mouse', json.encode(modify({'x': '$x2', 'y': '$y2'})));
  }

  static List<Peer> peers() {
    try {
      var str = getByName('peers');
      if (str == "") return [];
      List<dynamic> peers = json.decode(str);
      return peers
          .map((s) => s as List<dynamic>)
          .map((s) =>
              Peer.fromJson(s[0] as String, s[1] as Map<String, dynamic>))
          .toList();
    } catch (e) {
      print('peers(): $e');
    }
    return [];
  }

  static void connect(String id, {bool isFileTransfer = false}) {
    if (isFileTransfer) {
      setByName('connect_file_transfer', id);
    } else {
      FFI.chatModel.resetClientMode();
      setByName('connect', id);
    }
    FFI.id = id;
  }

  static Map<String, dynamic>? popEvent() {
    var s = getByName('event');
    if (s == '') return null;
    try {
      Map<String, dynamic> event = json.decode(s);
      return event;
    } catch (e) {
      print('popEvent(): $e');
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
    chatModel.close();
    if (FFI.imageModel.image != null && !isDesktop) {
      savePreference(id, cursorModel.x, cursorModel.y, canvasModel.x,
          canvasModel.y, canvasModel.scale, ffiModel.pi.currentDisplay);
    }
    id = "";
    setByName('close', '');
    imageModel.update(null);
    cursorModel.clear();
    ffiModel.clear();
    canvasModel.clear();
    resetModifiers();
  }

  static String getByName(String name, [String arg = '']) {
    return PlatformFFI.getByName(name, arg);
  }

  static void setByName(String name, [String value = '']) {
    PlatformFFI.setByName(name, value);
  }

  static handleMouse(Map<String, dynamic> evt) {
    var type = '';
    var isMove = false;
    switch (evt['type']) {
      case 'mousedown':
        type = 'down';
        break;
      case 'mouseup':
        type = 'up';
        break;
      case 'mousemove':
        isMove = true;
        break;
      default:
        return;
    }
    evt['type'] = type;
    var x = evt['x'];
    var y = evt['y'];
    if (isMove) {
      FFI.canvasModel.moveDesktopMouse(x, y);
    }
    final d = FFI.ffiModel.display;
    x -= FFI.canvasModel.x;
    y -= FFI.canvasModel.y;
    if (!isMove && (x < 0 || x > d.width || y < 0 || y > d.height)) {
      return;
    }
    x /= FFI.canvasModel.scale;
    y /= FFI.canvasModel.scale;
    x += d.x;
    y += d.y;
    if (type != '') {
      x = 0;
      y = 0;
    }
    evt['x'] = '${x.round()}';
    evt['y'] = '${y.round()}';
    var buttons = '';
    switch (evt['buttons']) {
      case 1:
        buttons = 'left';
        break;
      case 2:
        buttons = 'right';
        break;
      case 4:
        buttons = 'wheel';
        break;
    }
    evt['buttons'] = buttons;
    setByName('send_mouse', json.encode(evt));
  }

  static listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      PlatformFFI.startDesktopWebListener();
    } else {
      PlatformFFI.stopDesktopWebListener();
    }
  }

  static void setMethodCallHandler(FMethod callback) {
    PlatformFFI.setMethodCallHandler(callback);
  }

  static Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return await PlatformFFI.invokeMethod(method, arguments);
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
  String version = "";
  String username = "";
  String hostname = "";
  String platform = "";
  bool sasEnabled = false;
  int currentDisplay = 0;
  List<Display> displays = [];
}

void savePreference(String id, double xCursor, double yCursor, double xCanvas,
    double yCanvas, double scale, int currentDisplay) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  final p = Map<String, dynamic>();
  p['xCursor'] = xCursor;
  p['yCursor'] = yCursor;
  p['xCanvas'] = xCanvas;
  p['yCanvas'] = yCanvas;
  p['scale'] = scale;
  p['currentDisplay'] = currentDisplay;
  prefs.setString('peer' + id, json.encode(p));
}

Future<Map<String, dynamic>?> getPreference(String id) async {
  if (!isDesktop) return null;
  SharedPreferences prefs = await SharedPreferences.getInstance();
  var p = prefs.getString('peer' + id);
  if (p == null) return null;
  Map<String, dynamic> m = json.decode(p);
  return m;
}

void removePreference(String id) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  prefs.remove('peer' + id);
}

void initializeCursorAndCanvas() async {
  var p = await getPreference(FFI.id);
  int currentDisplay = 0;
  if (p != null) {
    currentDisplay = p['currentDisplay'];
  }
  if (p == null || currentDisplay != FFI.ffiModel.pi.currentDisplay) {
    FFI.cursorModel
        .updateDisplayOrigin(FFI.ffiModel.display.x, FFI.ffiModel.display.y);
    return;
  }
  double xCursor = p['xCursor'];
  double yCursor = p['yCursor'];
  double xCanvas = p['xCanvas'];
  double yCanvas = p['yCanvas'];
  double scale = p['scale'];
  FFI.cursorModel.updateDisplayOriginWithCursor(
      FFI.ffiModel.display.x, FFI.ffiModel.display.y, xCursor, yCursor);
  FFI.canvasModel.update(xCanvas, yCanvas, scale);
}

String translate(String name) {
  if (name.startsWith('Failed to') && name.contains(': ')) {
    return name.split(': ').map((x) => translate(x)).join(': ');
  }
  var a = 'translate';
  var b = '{"locale": "$localeName", "text": "$name"}';
  return FFI.getByName(a, b);
}
