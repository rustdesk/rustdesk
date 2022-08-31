import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tuple/tuple.dart';

import '../common.dart';
import '../common/shared_state.dart';
import '../mobile/widgets/dialog.dart';
import '../mobile/widgets/overlay.dart';
import 'peer_model.dart';
import 'platform_model.dart';

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
  WeakReference<FFI> parent;

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

  FfiModel(this.parent) {
    clear();
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

  void setConnectionType(String peerId, bool secure, bool direct) {
    _secure = secure;
    _direct = direct;
    try {
      var connectionType = ConnectionTypeState.find(peerId);
      connectionType.setSecure(secure);
      connectionType.setDirect(direct);
    } catch (e) {
      //
    }
  }

  Image? getConnectionImage() {
    if (secure == null || direct == null) {
      return null;
    } else {
      final icon =
          '${secure == true ? "secure" : "insecure"}${direct == true ? "" : "_relay"}';
      return Image.asset('assets/$icon.png', width: 48, height: 48);
    }
  }

  void clearPermissions() {
    _inputBlocked = false;
    _permissions.clear();
  }

  void Function(Map<String, dynamic>) startEventListener(String peerId) {
    return (evt) {
      var name = evt['name'];
      if (name == 'msgbox') {
        handleMsgBox(evt, peerId);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt, peerId);
      } else if (name == 'connection_ready') {
        setConnectionType(
            peerId, evt['secure'] == 'true', evt['direct'] == 'true');
      } else if (name == 'switch_display') {
        handleSwitchDisplay(evt);
      } else if (name == 'cursor_data') {
        parent.target?.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        parent.target?.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        parent.target?.cursorModel.updateCursorPosition(evt);
      } else if (name == 'clipboard') {
        Clipboard.setData(ClipboardData(text: evt['content']));
      } else if (name == 'permission') {
        parent.target?.ffiModel.updatePermission(evt);
      } else if (name == 'chat_client_mode') {
        parent.target?.chatModel
            .receive(ChatModel.clientModeID, evt['text'] ?? "");
      } else if (name == 'chat_server_mode') {
        parent.target?.chatModel
            .receive(int.parse(evt['id'] as String), evt['text'] ?? "");
      } else if (name == 'file_dir') {
        parent.target?.fileModel.receiveFileDir(evt);
      } else if (name == 'job_progress') {
        parent.target?.fileModel.tryUpdateJobProgress(evt);
      } else if (name == 'job_done') {
        parent.target?.fileModel.jobDone(evt);
      } else if (name == 'job_error') {
        parent.target?.fileModel.jobError(evt);
      } else if (name == 'override_file_confirm') {
        parent.target?.fileModel.overrideFileConfirm(evt);
      } else if (name == 'load_last_job') {
        parent.target?.fileModel.loadLastJob(evt);
      } else if (name == 'update_folder_files') {
        parent.target?.fileModel.updateFolderFiles(evt);
      } else if (name == 'try_start_without_auth') {
        parent.target?.serverModel.loginRequest(evt);
      } else if (name == 'on_client_authorized') {
        parent.target?.serverModel.onClientAuthorized(evt);
      } else if (name == 'on_client_remove') {
        parent.target?.serverModel.onClientRemove(evt);
      } else if (name == 'update_quality_status') {
        parent.target?.qualityMonitorModel.updateQualityStatus(evt);
      } else if (name == 'update_block_input_state') {
        updateBlockInputState(evt, peerId);
      } else if (name == 'update_privacy_mode') {
        updatePrivacyMode(evt, peerId);
      }
    };
  }

  /// Bind the event listener to receive events from the Rust core.
  void updateEventListener(String peerId) {
    final void Function(Map<String, dynamic>) cb = (evt) {
      var name = evt['name'];
      if (name == 'msgbox') {
        handleMsgBox(evt, peerId);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt, peerId);
      } else if (name == 'connection_ready') {
        parent.target?.ffiModel.setConnectionType(
            peerId, evt['secure'] == 'true', evt['direct'] == 'true');
      } else if (name == 'switch_display') {
        handleSwitchDisplay(evt);
      } else if (name == 'cursor_data') {
        parent.target?.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        parent.target?.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        parent.target?.cursorModel.updateCursorPosition(evt);
      } else if (name == 'clipboard') {
        Clipboard.setData(ClipboardData(text: evt['content']));
      } else if (name == 'permission') {
        parent.target?.ffiModel.updatePermission(evt);
      } else if (name == 'chat_client_mode') {
        parent.target?.chatModel
            .receive(ChatModel.clientModeID, evt['text'] ?? "");
      } else if (name == 'chat_server_mode') {
        parent.target?.chatModel
            .receive(int.parse(evt['id'] as String), evt['text'] ?? "");
      } else if (name == 'file_dir') {
        parent.target?.fileModel.receiveFileDir(evt);
      } else if (name == 'job_progress') {
        parent.target?.fileModel.tryUpdateJobProgress(evt);
      } else if (name == 'job_done') {
        parent.target?.fileModel.jobDone(evt);
      } else if (name == 'job_error') {
        parent.target?.fileModel.jobError(evt);
      } else if (name == 'override_file_confirm') {
        parent.target?.fileModel.overrideFileConfirm(evt);
      } else if (name == 'load_last_job') {
        parent.target?.fileModel.loadLastJob(evt);
      } else if (name == 'update_folder_files') {
        parent.target?.fileModel.updateFolderFiles(evt);
      } else if (name == 'try_start_without_auth') {
        parent.target?.serverModel.loginRequest(evt);
      } else if (name == 'on_client_authorized') {
        parent.target?.serverModel.onClientAuthorized(evt);
      } else if (name == 'on_client_remove') {
        parent.target?.serverModel.onClientRemove(evt);
      } else if (name == 'update_quality_status') {
        parent.target?.qualityMonitorModel.updateQualityStatus(evt);
      } else if (name == 'update_block_input_state') {
        updateBlockInputState(evt, peerId);
      } else if (name == 'update_privacy_mode') {
        updatePrivacyMode(evt, peerId);
      }
    };
    platformFFI.setEventCallback(cb);
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
      parent.target?.cursorModel.updateDisplayOrigin(_display.x, _display.y);

    // remote is mobile, and orientation changed
    if ((_display.width > _display.height) != oldOrientation) {
      gFFI.canvasModel.updateViewStyle();
    }
    notifyListeners();
  }

  /// Handle the message box event based on [evt] and [id].
  void handleMsgBox(Map<String, dynamic> evt, String id) {
    if (parent.target == null) return;
    final dialogManager = parent.target!.dialogManager;
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id, dialogManager);
    } else if (type == 'input-password') {
      enterPasswordDialog(id, dialogManager);
    } else if (type == 'restarting') {
      showMsgBox(id, type, title, text, false, dialogManager, hasCancel: false);
    } else {
      var hasRetry = evt['hasRetry'] == 'true';
      showMsgBox(id, type, title, text, hasRetry, dialogManager);
    }
  }

  /// Show a message box with [type], [title] and [text].
  void showMsgBox(String id, String type, String title, String text,
      bool hasRetry, OverlayDialogManager dialogManager,
      {bool? hasCancel}) {
    msgBox(type, title, text, dialogManager, hasCancel: hasCancel);
    _timer?.cancel();
    if (hasRetry) {
      _timer = Timer(Duration(seconds: _reconnects), () {
        bind.sessionReconnect(id: id);
        clearPermissions();
        dialogManager.showLoading(translate('Connecting...'),
            onCancel: closeConnection);
      });
      _reconnects *= 2;
    } else {
      _reconnects = 1;
    }
  }

  /// Handle the peer info event based on [evt].
  void handlePeerInfo(Map<String, dynamic> evt, String peerId) async {
    parent.target?.dialogManager.dismissAll();
    _pi.version = evt['version'];
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == "true";
    _pi.currentDisplay = int.parse(evt['current_display']);

    try {
      CurrentDisplayState.find(peerId).value = _pi.currentDisplay;
    } catch (e) {
      //
    }

    if (isPeerAndroid) {
      _touchMode = true;
      if (parent.target?.ffiModel.permissions['keyboard'] != false) {
        Timer(Duration(milliseconds: 100), showMobileActionsOverlay);
      }
    } else {
      _touchMode =
          await bind.sessionGetOption(id: peerId, arg: "touch-mode") != '';
    }

    if (evt['is_file_transfer'] == "true") {
      // TODO is file transfer
      parent.target?.fileModel.onReady();
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
        parent.target?.dialogManager.showLoading(
            translate('Connected, waiting for image...'),
            onCancel: closeConnection);
        _waitForImage = true;
        _reconnects = 1;
      }
    }
    notifyListeners();
  }

  updateBlockInputState(Map<String, dynamic> evt, String peerId) {
    _inputBlocked = evt['input_state'] == 'on';
    notifyListeners();
    try {
      BlockInputState.find(peerId).value = evt['input_state'] == 'on';
    } catch (e) {
      //
    }
  }

  updatePrivacyMode(Map<String, dynamic> evt, String peerId) {
    notifyListeners();
    try {
      PrivacyModeState.find(peerId).value =
          bind.sessionGetToggleOptionSync(id: peerId, arg: 'privacy-mode');
    } catch (e) {
      //
    }
  }
}

class ImageModel with ChangeNotifier {
  ui.Image? _image;

  ui.Image? get image => _image;

  String _id = "";

  WeakReference<FFI> parent;

  ImageModel(this.parent);

  void onRgba(Uint8List rgba, double tabBarHeight) {
    if (_waitForImage) {
      _waitForImage = false;
      parent.target?.dialogManager.dismissAll();
    }
    final pid = parent.target?.id;
    ui.decodeImageFromPixels(
        rgba,
        parent.target?.ffiModel.display.width ?? 0,
        parent.target?.ffiModel.display.height ?? 0,
        isWeb ? ui.PixelFormat.rgba8888 : ui.PixelFormat.bgra8888, (image) {
      if (parent.target?.id != pid) return;
      try {
        // my throw exception, because the listener maybe already dispose
        update(image, tabBarHeight);
      } catch (e) {
        print('update image: $e');
      }
    });
  }

  void update(ui.Image? image, double tabBarHeight) {
    if (_image == null && image != null) {
      if (isWebDesktop || isDesktop) {
        parent.target?.canvasModel.updateViewStyle();
        parent.target?.canvasModel.updateScrollStyle();
      } else {
        final size = MediaQueryData.fromWindow(ui.window).size;
        final canvasWidth = size.width;
        final canvasHeight = size.height - tabBarHeight;
        final xscale = canvasWidth / image.width;
        final yscale = canvasHeight / image.height;
        parent.target?.canvasModel.scale = min(xscale, yscale);
      }
      if (parent.target != null) {
        initializeCursorAndCanvas(parent.target!);
      }
      Future.delayed(Duration(milliseconds: 1), () {
        if (parent.target?.ffiModel.isPeerAndroid ?? false) {
          bind.sessionPeerOption(id: _id, name: "view-style", value: "shrink");
          parent.target?.canvasModel.updateViewStyle();
        }
      });
    }
    _image = image;
    if (image != null) notifyListeners();
  }

  // mobile only
  // for desktop, height should minus tabbar height
  double get maxScale {
    if (_image == null) return 1.5;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return max(1.5, max(xscale, yscale));
  }

  // mobile only
  // for desktop, height should minus tabbar height
  double get minScale {
    if (_image == null) return 1.5;
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return min(xscale, yscale) / 1.5;
  }
}

enum ScrollStyle {
  scrollbar,
  scrollauto,
}

class CanvasModel with ChangeNotifier {
  // scroll offset x percent
  double _scrollX = 0.0;
  // scroll offset y percent
  double _scrollY = 0.0;
  double _x = 0;
  double _y = 0;
  double _scale = 1.0;
  double _tabBarHeight = 0.0;
  String id = ""; // TODO multi canvas model
  ScrollStyle _scrollStyle = ScrollStyle.scrollauto;

  WeakReference<FFI> parent;

  CanvasModel(this.parent);

  double get x => _x;
  double get y => _y;
  double get scale => _scale;
  ScrollStyle get scrollStyle => _scrollStyle;

  setScrollPercent(double x, double y) {
    _scrollX = x;
    _scrollY = y;
  }

  double get scrollX => _scrollX;
  double get scrollY => _scrollY;

  set tabBarHeight(double h) => _tabBarHeight = h;
  double get tabBarHeight => _tabBarHeight;

  void updateViewStyle() async {
    final style = await bind.sessionGetOption(id: id, arg: 'view-style');
    if (style == null) {
      return;
    }

    _scale = 1.0;
    if (style == 'adaptive') {
      final s1 = size.width / (parent.target?.ffiModel.display.width ?? 720);
      final s2 = size.height / (parent.target?.ffiModel.display.height ?? 1280);
      _scale = s1 < s2 ? s1 : s2;
    }

    _x = (size.width - getDisplayWidth() * _scale) / 2;
    _y = (size.height - getDisplayHeight() * _scale) / 2;
    notifyListeners();
  }

  updateScrollStyle() async {
    final style = await bind.sessionGetOption(id: id, arg: 'scroll-style');
    if (style == 'scrollbar') {
      _scrollStyle = ScrollStyle.scrollbar;
      _scrollX = 0.0;
      _scrollY = 0.0;
    } else {
      _scrollStyle = ScrollStyle.scrollauto;
    }
    notifyListeners();
  }

  void update(double x, double y, double scale) {
    _x = x;
    _y = y;
    _scale = scale;
    notifyListeners();
  }

  int getDisplayWidth() {
    return parent.target?.ffiModel.display.width ?? 1080;
  }

  int getDisplayHeight() {
    return parent.target?.ffiModel.display.height ?? 720;
  }

  Size get size {
    final size = MediaQueryData.fromWindow(ui.window).size;
    return Size(size.width, size.height - _tabBarHeight);
  }

  void moveDesktopMouse(double x, double y) {
    // On mobile platforms, move the canvas with the cursor.
    //if (!isDesktop) {
    final dw = getDisplayWidth() * _scale;
    final dh = getDisplayHeight() * _scale;
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
    //}
    parent.target?.cursorModel.moveLocal(x, y);
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
    if (isWebDesktop) {
      updateViewStyle();
    } else {
      final size = MediaQueryData.fromWindow(ui.window).size;
      final canvasWidth = size.width;
      final canvasHeight = size.height - _tabBarHeight;
      _x = (canvasWidth - getDisplayWidth() * _scale) / 2;
      _y = (canvasHeight - getDisplayHeight() * _scale) / 2;
    }
    notifyListeners();
  }

  void panY(double dy) {
    _y += dy;
    notifyListeners();
  }

  void updateScale(double v) {
    if (parent.target?.imageModel.image == null) return;
    final offset = parent.target?.cursorModel.offset ?? Offset(0, 0);
    var r = parent.target?.cursorModel.getVisibleRect() ?? Rect.zero;
    final px0 = (offset.dx - r.left) * _scale;
    final py0 = (offset.dy - r.top) * _scale;
    _scale *= v;
    final maxs = parent.target?.imageModel.maxScale ?? 1;
    final mins = parent.target?.imageModel.minScale ?? 1;
    if (_scale > maxs) _scale = maxs;
    if (_scale < mins) _scale = mins;
    r = parent.target?.cursorModel.getVisibleRect() ?? Rect.zero;
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
  String id = ""; // TODO multi cursor model
  WeakReference<FFI> parent;

  ui.Image? get image => _image;

  double get x => _x - _displayOriginX;

  double get y => _y - _displayOriginY;

  Offset get offset => Offset(_x, _y);

  double get hotx => _hotx;

  double get hoty => _hoty;

  CursorModel(this.parent);

  // remote physical display coordinate
  Rect getVisibleRect() {
    final size = MediaQueryData.fromWindow(ui.window).size;
    final xoffset = parent.target?.canvasModel.x ?? 0;
    final yoffset = parent.target?.canvasModel.y ?? 0;
    final scale = parent.target?.canvasModel.scale ?? 1;
    final x0 = _displayOriginX - xoffset / scale;
    final y0 = _displayOriginY - yoffset / scale;
    return Rect.fromLTWH(x0, y0, size.width / scale, size.height / scale);
  }

  double adjustForKeyboard() {
    final m = MediaQueryData.fromWindow(ui.window);
    var keyboardHeight = m.viewInsets.bottom;
    final size = m.size;
    if (keyboardHeight < 100) return 0;
    final s = parent.target?.canvasModel.scale ?? 1.0;
    final thresh = (size.height - keyboardHeight) / 2;
    var h = (_y - getVisibleRect().top) * s; // local physical display height
    return h - thresh;
  }

  void touch(double x, double y, MouseButtons button) {
    moveLocal(x, y);
    parent.target?.moveMouse(_x, _y);
    parent.target?.tap(button);
  }

  void move(double x, double y) {
    moveLocal(x, y);
    parent.target?.moveMouse(_x, _y);
  }

  void moveLocal(double x, double y) {
    final scale = parent.target?.canvasModel.scale ?? 1.0;
    final xoffset = parent.target?.canvasModel.x ?? 0;
    final yoffset = parent.target?.canvasModel.y ?? 0;
    _x = (x - xoffset) / scale + _displayOriginX;
    _y = (y - yoffset) / scale + _displayOriginY;
    notifyListeners();
  }

  void reset() {
    _x = _displayOriginX;
    _y = _displayOriginY;
    parent.target?.moveMouse(_x, _y);
    parent.target?.canvasModel.clear(true);
    notifyListeners();
  }

  void updatePan(double dx, double dy, bool touchMode) {
    if (parent.target?.imageModel.image == null) return;
    if (touchMode) {
      final scale = parent.target?.canvasModel.scale ?? 1.0;
      _x += dx / scale;
      _y += dy / scale;
      parent.target?.moveMouse(_x, _y);
      notifyListeners();
      return;
    }
    final scale = parent.target?.canvasModel.scale ?? 1.0;
    dx /= scale;
    dy /= scale;
    final r = getVisibleRect();
    var cx = r.center.dx;
    var cy = r.center.dy;
    var tryMoveCanvasX = false;
    if (dx > 0) {
      final maxCanvasCanMove = _displayOriginX +
          (parent.target?.imageModel.image!.width ?? 1280) -
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
          (parent.target?.imageModel.image!.height ?? 720) -
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
      parent.target?.canvasModel.panX(-dx);
    }
    if (tryMoveCanvasY && dy != 0) {
      parent.target?.canvasModel.panY(-dy);
    }

    parent.target?.moveMouse(_x, _y);
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
    var pid = parent.target?.id;
    ui.decodeImageFromPixels(rgba, width, height, ui.PixelFormat.rgba8888,
        (image) {
      if (parent.target?.id != pid) return;
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

  /// Update the cursor position.
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
    parent.target?.moveMouse(x, y);
    parent.target?.canvasModel.resetOffset();
    notifyListeners();
  }

  void updateDisplayOriginWithCursor(
      double x, double y, double xCursor, double yCursor) {
    _displayOriginX = x;
    _displayOriginY = y;
    _x = xCursor;
    _y = yCursor;
    parent.target?.moveMouse(x, y);
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
  WeakReference<FFI> parent;

  QualityMonitorModel(this.parent);
  var _show = false;
  final _data = QualityMonitorData();

  bool get show => _show;
  QualityMonitorData get data => _data;

  checkShowQualityMonitor(String id) async {
    final show = await bind.sessionGetToggleOption(
            id: id, arg: 'show-quality-monitor') ==
        true;
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

/// Mouse button enum.
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

/// FFI class for communicating with the Rust core.
class FFI {
  var id = "";
  var shift = false;
  var ctrl = false;
  var alt = false;
  var command = false;
  var version = "";

  /// dialogManager use late to ensure init after main page binding [globalKey]
  late final dialogManager = OverlayDialogManager();

  late final ImageModel imageModel; // session
  late final FfiModel ffiModel; // session
  late final CursorModel cursorModel; // session
  late final CanvasModel canvasModel; // session
  late final ServerModel serverModel; // global
  late final ChatModel chatModel; // session
  late final FileModel fileModel; // session
  late final AbModel abModel; // global
  late final UserModel userModel; // global
  late final QualityMonitorModel qualityMonitorModel; // session

  FFI() {
    this.imageModel = ImageModel(WeakReference(this));
    this.ffiModel = FfiModel(WeakReference(this));
    this.cursorModel = CursorModel(WeakReference(this));
    this.canvasModel = CanvasModel(WeakReference(this));
    this.serverModel = ServerModel(WeakReference(this)); // use global FFI
    this.chatModel = ChatModel(WeakReference(this));
    this.fileModel = FileModel(WeakReference(this));
    this.abModel = AbModel(WeakReference(this));
    this.userModel = UserModel(WeakReference(this));
    this.qualityMonitorModel = QualityMonitorModel(WeakReference(this));
  }

  /// Send a mouse tap event(down and up).
  void tap(MouseButtons button) {
    sendMouse('down', button);
    sendMouse('up', button);
  }

  /// Send scroll event with scroll distance [y].
  void scroll(int y) {
    bind.sessionSendMouse(
        id: id,
        msg: json
            .encode(modify({'id': id, 'type': 'wheel', 'y': y.toString()})));
  }

  /// Reconnect to the remote peer.
  // static void reconnect() {
  //   setByName('reconnect');
  //   parent.target?.ffiModel.clearPermissions();
  // }

  /// Reset key modifiers to false, including [shift], [ctrl], [alt] and [command].
  void resetModifiers() {
    shift = ctrl = alt = command = false;
  }

  /// Modify the given modifier map [evt] based on current modifier key status.
  Map<String, String> modify(Map<String, String> evt) {
    if (ctrl) evt['ctrl'] = 'true';
    if (shift) evt['shift'] = 'true';
    if (alt) evt['alt'] = 'true';
    if (command) evt['command'] = 'true';
    return evt;
  }

  /// Send mouse press event.
  void sendMouse(String type, MouseButtons button) {
    if (!ffiModel.keyboard()) return;
    bind.sessionSendMouse(
        id: id,
        msg: json.encode(modify({'type': type, 'buttons': button.value})));
  }

  /// Send key stroke event.
  /// [down] indicates the key's state(down or up).
  /// [press] indicates a click event(down and up).
  void inputKey(String name, {bool? down, bool? press}) {
    if (!ffiModel.keyboard()) return;
    // final Map<String, String> out = Map();
    // out['name'] = name;
    // // default: down = false
    // if (down == true) {
    //   out['down'] = "true";
    // }
    // // default: press = true
    // if (press != false) {
    //   out['press'] = "true";
    // }
    // setByName('input_key', json.encode(modify(out)));
    // TODO id
    bind.sessionInputKey(
        id: id,
        name: name,
        down: down ?? false,
        press: press ?? true,
        alt: alt,
        ctrl: ctrl,
        shift: shift,
        command: command);
  }

  /// Send mouse movement event with distance in [x] and [y].
  void moveMouse(double x, double y) {
    if (!ffiModel.keyboard()) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    bind.sessionSendMouse(
        id: id, msg: json.encode(modify({'x': '$x2', 'y': '$y2'})));
  }

  /// List the saved peers.
  Future<List<Peer>> peers() async {
    try {
      var str = await bind.mainGetRecentPeers();
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

  /// Connect with the given [id]. Only transfer file if [isFileTransfer], only port forward if [isPortForward].
  void connect(String id,
      {bool isFileTransfer = false,
      bool isPortForward = false,
      double tabBarHeight = 0.0}) {
    assert(!(isFileTransfer && isPortForward), "more than one connect type");
    if (isFileTransfer) {
      id = 'ft_${id}';
    } else if (isPortForward) {
      id = 'pf_${id}';
    } else {
      chatModel.resetClientMode();
      canvasModel.id = id;
      imageModel._id = id;
      cursorModel.id = id;
    }
    // ignore: unused_local_variable
    final addRes = bind.sessionAddSync(
        id: id, isFileTransfer: isFileTransfer, isPortForward: isPortForward);
    final stream = bind.sessionStart(id: id);
    final cb = ffiModel.startEventListener(id);
    () async {
      await for (final message in stream) {
        if (message is Event) {
          try {
            Map<String, dynamic> event = json.decode(message.field0);
            cb(event);
          } catch (e) {
            print('json.decode fail(): $e');
          }
        } else if (message is Rgba) {
          imageModel.onRgba(message.field0, tabBarHeight);
        }
      }
    }();
    // every instance will bind a stream
    this.id = id;
    if (isFileTransfer) {
      this.fileModel.initFileFetcher();
    }
  }

  /// Login with [password], choose if the client should [remember] it.
  void login(String id, String password, bool remember) {
    bind.sessionLogin(id: id, password: password, remember: remember);
  }

  /// Close the remote session.
  Future<void> close() async {
    chatModel.close();
    if (imageModel.image != null && !isWebDesktop) {
      await savePreference(id, cursorModel.x, cursorModel.y, canvasModel.x,
          canvasModel.y, canvasModel.scale, ffiModel.pi.currentDisplay);
    }
    bind.sessionClose(id: id);
    id = "";
    imageModel.update(null, 0.0);
    cursorModel.clear();
    ffiModel.clear();
    canvasModel.clear();
    resetModifiers();
    debugPrint("model $id closed");
  }

  /// Send **get** command to the Rust core based on [name] and [arg].
  /// Return the result as a string.
  // String getByName(String name, [String arg = '']) {
  //   return platformFFI.getByName(name, arg);
  // }

  /// Send **set** command to the Rust core based on [name] and [value].
  // void setByName(String name, [String value = '']) {
  //   platformFFI.setByName(name, value);
  // }

  handleMouse(Map<String, dynamic> evt, {double tabBarHeight = 0.0}) {
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
    var y = max(0.0, (evt['y'] as double) - tabBarHeight);
    if (isMove) {
      canvasModel.moveDesktopMouse(x, y);
    }
    final d = ffiModel.display;
    if (canvasModel.scrollStyle == ScrollStyle.scrollbar) {
      final imageWidth = d.width * canvasModel.scale;
      final imageHeight = d.height * canvasModel.scale;
      x += imageWidth * canvasModel.scrollX;
      y += imageHeight * canvasModel.scrollY;

      // boxed size is a center widget
      if (canvasModel.size.width > imageWidth) {
        x -= ((canvasModel.size.width - imageWidth) / 2);
      }
      if (canvasModel.size.height > imageHeight) {
        y -= ((canvasModel.size.height - imageHeight) / 2);
      }
    } else {
      x -= canvasModel.x;
      y -= canvasModel.y;
    }

    if (!isMove && (x < 0 || x > d.width || y < 0 || y > d.height)) {
      return;
    }
    x /= canvasModel.scale;
    y /= canvasModel.scale;
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
    bind.sessionSendMouse(id: id, msg: json.encode(evt));
  }

  listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      platformFFI.startDesktopWebListener();
    } else {
      platformFFI.stopDesktopWebListener();
    }
  }

  void setMethodCallHandler(FMethod callback) {
    platformFFI.setMethodCallHandler(callback);
  }

  Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return await platformFFI.invokeMethod(method, arguments);
  }

  Future<List<String>> getAudioInputs() async {
    return await bind.mainGetSoundInputs();
  }

  Future<String> getDefaultAudioInput() async {
    final input = await bind.mainGetOption(key: 'audio-input');
    if (input.isEmpty && Platform.isWindows) {
      return "System Sound";
    }
    return input;
  }

  void setDefaultAudioInput(String input) {
    bind.mainSetOption(key: 'audio-input', value: input);
  }

  Future<Map<String, String>> getHttpHeaders() async {
    return {
      "Authorization":
          "Bearer " + await bind.mainGetLocalOption(key: "access_token")
    };
  }
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

Future<void> savePreference(String id, double xCursor, double yCursor,
    double xCanvas, double yCanvas, double scale, int currentDisplay) async {
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
  if (!isWebDesktop) return null;
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

void initializeCursorAndCanvas(FFI ffi) async {
  var p = await getPreference(ffi.id);
  int currentDisplay = 0;
  if (p != null) {
    currentDisplay = p['currentDisplay'];
  }
  if (p == null || currentDisplay != ffi.ffiModel.pi.currentDisplay) {
    ffi.cursorModel
        .updateDisplayOrigin(ffi.ffiModel.display.x, ffi.ffiModel.display.y);
    return;
  }
  double xCursor = p['xCursor'];
  double yCursor = p['yCursor'];
  double xCanvas = p['xCanvas'];
  double yCanvas = p['yCanvas'];
  double scale = p['scale'];
  ffi.cursorModel.updateDisplayOriginWithCursor(
      ffi.ffiModel.display.x, ffi.ffiModel.display.y, xCursor, yCursor);
  ffi.canvasModel.update(xCanvas, yCanvas, scale);
}

/// Translate text based on the pre-defined dictionary.
/// note: params [FFI?] can be used to replace global FFI implementation
/// for example: during global initialization, gFFI not exists yet.
// String translate(String name, {FFI? ffi}) {
//   if (name.startsWith('Failed to') && name.contains(': ')) {
//     return name.split(': ').map((x) => translate(x)).join(': ');
//   }
//   var a = 'translate';
//   var b = '{"locale": "$localeName", "text": "$name"}';
//
//   return (ffi ?? gFFI).getByName(a, b);
// }
