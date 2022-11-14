import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:tuple/tuple.dart';
import 'package:image/image.dart' as img2;
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../common.dart';
import '../common/shared_state.dart';
import '../utils/image.dart' as img;
import '../mobile/widgets/dialog.dart';
import 'input_model.dart';
import 'platform_model.dart';

typedef HandleMsgBox = Function(Map<String, dynamic> evt, String id);
final _waitForImage = <String, bool>{};

class FfiModel with ChangeNotifier {
  PeerInfo _pi = PeerInfo();
  Display _display = Display();

  var _inputBlocked = false;
  final _permissions = <String, bool>{};
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

  bool get isPeerAndroid => _pi.platform == 'Android';

  set inputBlocked(v) {
    _inputBlocked = v;
  }

  FfiModel(this.parent) {
    clear();
  }

  toggleTouchMode() {
    if (!isPeerAndroid) {
      _touchMode = !_touchMode;
      notifyListeners();
    }
  }

  updatePermission(Map<String, dynamic> evt, String id) {
    evt.forEach((k, v) {
      if (k == 'name' || k.isEmpty) return;
      _permissions[k] = v == 'true';
    });
    // Only inited at remote page
    if (desktopType == DesktopType.remote) {
      KeyboardEnabledState.find(id).value = _permissions['keyboard'] != false;
    }
    debugPrint('$_permissions');
    notifyListeners();
  }

  bool keyboard() => _permissions['keyboard'] != false;

  clear() {
    _pi = PeerInfo();
    _display = Display();
    _secure = null;
    _direct = null;
    _inputBlocked = false;
    _timer?.cancel();
    _timer = null;
    clearPermissions();
  }

  setConnectionType(String peerId, bool secure, bool direct) {
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

  Widget? getConnectionImage() {
    if (secure == null || direct == null) {
      return null;
    } else {
      final icon =
          '${secure == true ? 'secure' : 'insecure'}${direct == true ? '' : '_relay'}';
      return SvgPicture.asset('assets/$icon.svg', width: 48, height: 48);
    }
  }

  clearPermissions() {
    _inputBlocked = false;
    _permissions.clear();
  }

  StreamEventHandler startEventListener(String peerId) {
    return (evt) async {
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
        await parent.target?.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        await parent.target?.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        await parent.target?.cursorModel.updateCursorPosition(evt, peerId);
      } else if (name == 'clipboard') {
        Clipboard.setData(ClipboardData(text: evt['content']));
      } else if (name == 'permission') {
        parent.target?.ffiModel.updatePermission(evt, peerId);
      } else if (name == 'chat_client_mode') {
        parent.target?.chatModel
            .receive(ChatModel.clientModeID, evt['text'] ?? '');
      } else if (name == 'chat_server_mode') {
        parent.target?.chatModel
            .receive(int.parse(evt['id'] as String), evt['text'] ?? '');
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
      } else if (name == 'add_connection') {
        parent.target?.serverModel.addConnection(evt);
      } else if (name == 'on_client_remove') {
        parent.target?.serverModel.onClientRemove(evt);
      } else if (name == 'update_quality_status') {
        parent.target?.qualityMonitorModel.updateQualityStatus(evt);
      } else if (name == 'update_block_input_state') {
        updateBlockInputState(evt, peerId);
      } else if (name == 'update_privacy_mode') {
        updatePrivacyMode(evt, peerId);
      } else if (name == 'new_connection') {
        var arg = evt['peer_id'].toString();
        if (arg.startsWith(kUniLinksPrefix)) {
          parseRustdeskUri(arg);
        } else {
          Future.delayed(Duration.zero, () {
            rustDeskWinManager.newRemoteDesktop(arg);
          });
        }
      } else if (name == 'alias') {
        handleAliasChanged(evt);
      }
    };
  }

  /// Bind the event listener to receive events from the Rust core.
  updateEventListener(String peerId) {
    platformFFI.setEventCallback(startEventListener(peerId));
  }

  handleAliasChanged(Map<String, dynamic> evt) {
    final rxAlias = PeerStringOption.find(evt['id'], 'alias');
    if (rxAlias.value != evt['alias']) {
      rxAlias.value = evt['alias'];
    }
  }

  handleSwitchDisplay(Map<String, dynamic> evt) {
    final oldOrientation = _display.width > _display.height;
    var old = _pi.currentDisplay;
    _pi.currentDisplay = int.parse(evt['display']);
    _display.x = double.parse(evt['x']);
    _display.y = double.parse(evt['y']);
    _display.width = int.parse(evt['width']);
    _display.height = int.parse(evt['height']);
    if (old != _pi.currentDisplay) {
      parent.target?.cursorModel.updateDisplayOrigin(_display.x, _display.y);
    }

    // remote is mobile, and orientation changed
    if ((_display.width > _display.height) != oldOrientation) {
      gFFI.canvasModel.updateViewStyle();
    }
    parent.target?.recordingModel.onSwitchDisplay();
    notifyListeners();
  }

  /// Handle the message box event based on [evt] and [id].
  handleMsgBox(Map<String, dynamic> evt, String id) {
    if (parent.target == null) return;
    final dialogManager = parent.target!.dialogManager;
    final type = evt['type'];
    final title = evt['title'];
    final text = evt['text'];
    final link = evt['link'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id, dialogManager);
    } else if (type == 'input-password') {
      enterPasswordDialog(id, dialogManager);
    } else if (type == 'restarting') {
      showMsgBox(id, type, title, text, link, false, dialogManager,
          hasCancel: false);
    } else {
      var hasRetry = evt['hasRetry'] == 'true';
      showMsgBox(id, type, title, text, link, hasRetry, dialogManager);
    }
  }

  /// Show a message box with [type], [title] and [text].
  showMsgBox(String id, String type, String title, String text, String link,
      bool hasRetry, OverlayDialogManager dialogManager,
      {bool? hasCancel}) {
    msgBox(type, title, text, link, dialogManager, hasCancel: hasCancel);
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
  handlePeerInfo(Map<String, dynamic> evt, String peerId) async {
    // recent peer updated by handle_peer_info(ui_session_interface.rs) --> handle_peer_info(client.rs) --> save_config(client.rs)
    bind.mainLoadRecentPeers();

    parent.target?.dialogManager.dismissAll();
    _pi.version = evt['version'];
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == 'true';
    _pi.currentDisplay = int.parse(evt['current_display']);

    try {
      CurrentDisplayState.find(peerId).value = _pi.currentDisplay;
    } catch (e) {
      //
    }

    if (isPeerAndroid) {
      _touchMode = true;
      if (parent.target != null &&
          parent.target!.connType == ConnType.defaultConn &&
          parent.target!.ffiModel.permissions['keyboard'] != false) {
        Timer(
            const Duration(milliseconds: 100),
            () => parent.target!.dialogManager
                .showMobileActionsOverlay(ffi: parent.target!));
      }
    } else {
      _touchMode =
          await bind.sessionGetOption(id: peerId, arg: 'touch-mode') != '';
    }

    if (parent.target != null &&
        parent.target!.connType == ConnType.fileTransfer) {
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
      if (displays.isNotEmpty) {
        parent.target?.dialogManager.showLoading(
            translate('Connected, waiting for image...'),
            onCancel: closeConnection);
        _waitForImage[peerId] = true;
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

  String id = '';

  WeakReference<FFI> parent;

  ImageModel(this.parent);

  onRgba(Uint8List rgba) {
    if (_waitForImage[id]!) {
      _waitForImage[id] = false;
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
        update(image);
      } catch (e) {
        debugPrint('update image: $e');
      }
    });
  }

  update(ui.Image? image) async {
    if (_image == null && image != null) {
      if (isWebDesktop || isDesktop) {
        await parent.target?.canvasModel.updateViewStyle();
        await parent.target?.canvasModel.updateScrollStyle();
      } else {
        final size = MediaQueryData.fromWindow(ui.window).size;
        final canvasWidth = size.width;
        final canvasHeight = size.height;
        final xscale = canvasWidth / image.width;
        final yscale = canvasHeight / image.height;
        parent.target?.canvasModel.scale = min(xscale, yscale);
      }
      if (parent.target != null) {
        await initializeCursorAndCanvas(parent.target!);
      }
      if (parent.target?.ffiModel.isPeerAndroid ?? false) {
        bind.sessionPeerOption(id: id, name: 'view-style', value: 'adaptive');
        parent.target?.canvasModel.updateViewStyle();
      }
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

class ViewStyle {
  final String style;
  final double width;
  final double height;
  final int displayWidth;
  final int displayHeight;
  ViewStyle({
    this.style = '',
    this.width = 0.0,
    this.height = 0.0,
    this.displayWidth = 0,
    this.displayHeight = 0,
  });

  static int _double2Int(double v) => (v * 100).round().toInt();

  @override
  bool operator ==(Object other) =>
      other is ViewStyle &&
      other.runtimeType == runtimeType &&
      _innerEqual(other);

  bool _innerEqual(ViewStyle other) {
    return style == other.style &&
        ViewStyle._double2Int(other.width) == ViewStyle._double2Int(width) &&
        ViewStyle._double2Int(other.height) == ViewStyle._double2Int(height) &&
        other.displayWidth == displayWidth &&
        other.displayHeight == displayHeight;
  }

  @override
  int get hashCode => Object.hash(
        style,
        ViewStyle._double2Int(width),
        ViewStyle._double2Int(height),
        displayWidth,
        displayHeight,
      ).hashCode;

  double get scale {
    double s = 1.0;
    if (style == 'adaptive') {
      final s1 = width / displayWidth;
      final s2 = height / displayHeight;
      s = s1 < s2 ? s1 : s2;
    }
    return s;
  }
}

class CanvasModel with ChangeNotifier {
  // image offset of canvas
  double _x = 0;
  // image offset of canvas
  double _y = 0;
  // image scale
  double _scale = 1.0;
  // the tabbar over the image
  // double tabBarHeight = 0.0;
  // the window border's width
  // double windowBorderWidth = 0.0;
  // remote id
  String id = '';
  // scroll offset x percent
  double _scrollX = 0.0;
  // scroll offset y percent
  double _scrollY = 0.0;
  ScrollStyle _scrollStyle = ScrollStyle.scrollauto;
  ViewStyle _lastViewStyle = ViewStyle();

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

  updateViewStyle() async {
    final style = await bind.sessionGetOption(id: id, arg: 'view-style');
    if (style == null) {
      return;
    }
    final sizeWidth = size.width;
    final sizeHeight = size.height;
    final displayWidth = getDisplayWidth();
    final displayHeight = getDisplayHeight();
    final viewStyle = ViewStyle(
      style: style,
      width: sizeWidth,
      height: sizeHeight,
      displayWidth: displayWidth,
      displayHeight: displayHeight,
    );
    if (_lastViewStyle == viewStyle) {
      return;
    }
    _lastViewStyle = viewStyle;
    _scale = viewStyle.scale;
    _x = (sizeWidth - displayWidth * _scale) / 2;
    _y = (sizeHeight - displayHeight * _scale) / 2;
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

  update(double x, double y, double scale) {
    _x = x;
    _y = y;
    _scale = scale;
    notifyListeners();
  }

  int getDisplayWidth() {
    final defaultWidth = (isDesktop || isWebDesktop)
        ? kDesktopDefaultDisplayWidth
        : kMobileDefaultDisplayWidth;
    return parent.target?.ffiModel.display.width ?? defaultWidth;
  }

  int getDisplayHeight() {
    final defaultHeight = (isDesktop || isWebDesktop)
        ? kDesktopDefaultDisplayHeight
        : kMobileDefaultDisplayHeight;
    return parent.target?.ffiModel.display.height ?? defaultHeight;
  }

  double get windowBorderWidth => stateGlobal.windowBorderWidth;
  double get tabBarHeight => stateGlobal.tabBarHeight;

  Size get size {
    final size = MediaQueryData.fromWindow(ui.window).size;
    // If minimized, w or h may be negative here.
    double w = size.width - windowBorderWidth * 2;
    double h = size.height - tabBarHeight - windowBorderWidth * 2;
    return Size(w < 0 ? 0 : w, h < 0 ? 0 : h);
  }

  moveDesktopMouse(double x, double y) {
    // On mobile platforms, move the canvas with the cursor.
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

    // If keyboard is not permitted, do not move cursor when mouse is moving.
    if (parent.target != null && parent.target!.ffiModel.keyboard()) {
      // Draw cursor if is not desktop.
      if (!isDesktop) {
        parent.target!.cursorModel.moveLocal(x, y);
      } else {
        try {
          RemoteCursorMovedState.find(id).value = false;
        } catch (e) {
          //
        }
      }
    }
  }

  set scale(v) {
    _scale = v;
    notifyListeners();
  }

  panX(double dx) {
    _x += dx;
    notifyListeners();
  }

  resetOffset() {
    if (isWebDesktop) {
      updateViewStyle();
    } else {
      _x = (size.width - getDisplayWidth() * _scale) / 2;
      _y = (size.height - getDisplayHeight() * _scale) / 2;
    }
    notifyListeners();
  }

  panY(double dy) {
    _y += dy;
    notifyListeners();
  }

  updateScale(double v) {
    if (parent.target?.imageModel.image == null) return;
    final offset = parent.target?.cursorModel.offset ?? const Offset(0, 0);
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

  clear([bool notify = false]) {
    _x = 0;
    _y = 0;
    _scale = 1.0;
    if (notify) notifyListeners();
  }
}

// data for cursor
class CursorData {
  final String peerId;
  final int id;
  final img2.Image? image;
  double scale;
  Uint8List? data;
  double hotx;
  double hoty;
  final int width;
  final int height;

  CursorData({
    required this.peerId,
    required this.id,
    required this.image,
    required this.scale,
    required this.data,
    required this.hotx,
    required this.hoty,
    required this.width,
    required this.height,
  });

  int _doubleToInt(double v) => (v * 10e6).round().toInt();

  double _checkUpdateScale(double scale) {
    // Update data if scale changed.
    if (Platform.isWindows) {
      final tgtWidth = (width * scale).toInt();
      final tgtHeight = (width * scale).toInt();
      if (tgtWidth < kMinCursorSize || tgtHeight < kMinCursorSize) {
        double sw = kMinCursorSize.toDouble() / width;
        double sh = kMinCursorSize.toDouble() / height;
        scale = sw < sh ? sh : sw;
      }
      if (_doubleToInt(this.scale) != _doubleToInt(scale)) {
        data = img2
            .copyResize(
              image!,
              width: (width * scale).toInt(),
              height: (height * scale).toInt(),
            )
            .getBytes(format: img2.Format.bgra);
      }
    }
    this.scale = scale;
    if (hotx > 0 && hoty > 0) {
      // default cursor data
      hotx = (width * scale) / 2;
      hoty = (height * scale) / 2;
    }
    return scale;
  }

  String updateGetKey(double scale) {
    scale = _checkUpdateScale(scale);
    return '${peerId}_${id}_${_doubleToInt(width * scale)}_${_doubleToInt(height * scale)}';
  }
}

class CursorModel with ChangeNotifier {
  ui.Image? _image;
  ui.Image? _defaultImage;
  final _images = <int, Tuple3<ui.Image, double, double>>{};
  CursorData? _cache;
  final _defaultCacheId = -1;
  CursorData? _defaultCache;
  final _cacheMap = <int, CursorData>{};
  final _cacheKeys = <String>{};
  double _x = -10000;
  double _y = -10000;
  double _hotx = 0;
  double _hoty = 0;
  double _displayOriginX = 0;
  double _displayOriginY = 0;
  bool gotMouseControl = true;
  DateTime _lastPeerMouse = DateTime.now()
      .subtract(Duration(milliseconds: 2 * kMouseControlTimeoutMSec));
  String id = '';
  WeakReference<FFI> parent;

  ui.Image? get image => _image;
  ui.Image? get defaultImage => _defaultImage;
  CursorData? get cache => _cache;
  CursorData? get defaultCache => _getDefaultCache();

  double get x => _x - _displayOriginX;
  double get y => _y - _displayOriginY;

  Offset get offset => Offset(_x, _y);

  double get hotx => _hotx;
  double get hoty => _hoty;

  bool get isPeerControlProtected =>
      DateTime.now().difference(_lastPeerMouse).inMilliseconds <
      kMouseControlTimeoutMSec;

  CursorModel(this.parent) {
    _getDefaultImage();
    _getDefaultCache();
  }

  Set<String> get cachedKeys => _cacheKeys;
  addKey(String key) => _cacheKeys.add(key);

  Future<ui.Image?> _getDefaultImage() async {
    if (_defaultImage == null) {
      final defaultImg = defaultCursorImage!;
      // This function is called only one time, no need to care about the performance.
      Uint8List data = defaultImg.getBytes(format: img2.Format.rgba);
      _defaultImage = await img.decodeImageFromPixels(
          data, defaultImg.width, defaultImg.height, ui.PixelFormat.rgba8888);
    }
    return _defaultImage;
  }

  CursorData? _getDefaultCache() {
    if (_defaultCache == null) {
      Uint8List data;
      double scale = 1.0;
      double hotx = (defaultCursorImage!.width * scale) / 2;
      double hoty = (defaultCursorImage!.height * scale) / 2;
      if (Platform.isWindows) {
        data = defaultCursorImage!.getBytes(format: img2.Format.bgra);
      } else {
        data = Uint8List.fromList(img2.encodePng(defaultCursorImage!));
      }

      _defaultCache = CursorData(
        peerId: id,
        id: _defaultCacheId,
        image: defaultCursorImage?.clone(),
        scale: scale,
        data: data,
        hotx: hotx,
        hoty: hoty,
        width: defaultCursorImage!.width,
        height: defaultCursorImage!.height,
      );
    }
    return _defaultCache;
  }

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

  move(double x, double y) {
    moveLocal(x, y);
    parent.target?.inputModel.moveMouse(_x, _y);
  }

  moveLocal(double x, double y) {
    final scale = parent.target?.canvasModel.scale ?? 1.0;
    final xoffset = parent.target?.canvasModel.x ?? 0;
    final yoffset = parent.target?.canvasModel.y ?? 0;
    _x = (x - xoffset) / scale + _displayOriginX;
    _y = (y - yoffset) / scale + _displayOriginY;
    notifyListeners();
  }

  reset() {
    _x = _displayOriginX;
    _y = _displayOriginY;
    parent.target?.inputModel.moveMouse(_x, _y);
    parent.target?.canvasModel.clear(true);
    notifyListeners();
  }

  updatePan(double dx, double dy, bool touchMode) {
    if (parent.target?.imageModel.image == null) return;
    if (touchMode) {
      final scale = parent.target?.canvasModel.scale ?? 1.0;
      _x += dx / scale;
      _y += dy / scale;
      parent.target?.inputModel.moveMouse(_x, _y);
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

    parent.target?.inputModel.moveMouse(_x, _y);
    notifyListeners();
  }

  updateCursorData(Map<String, dynamic> evt) async {
    var id = int.parse(evt['id']);
    _hotx = double.parse(evt['hotx']);
    _hoty = double.parse(evt['hoty']);
    var width = int.parse(evt['width']);
    var height = int.parse(evt['height']);
    List<dynamic> colors = json.decode(evt['colors']);
    final rgba = Uint8List.fromList(colors.map((s) => s as int).toList());
    final image = await img.decodeImageFromPixels(
        rgba, width, height, ui.PixelFormat.rgba8888);
    _image = image;
    if (await _updateCache(image, id, width, height)) {
      _images[id] = Tuple3(image, _hotx, _hoty);
    } else {
      _hotx = 0;
      _hoty = 0;
    }
    try {
      // my throw exception, because the listener maybe already dispose
      notifyListeners();
    } catch (e) {
      debugPrint('WARNING: updateCursorId $id, without notifyListeners(). $e');
    }
  }

  Future<bool> _updateCache(ui.Image image, int id, int w, int h) async {
    ui.ImageByteFormat imgFormat = ui.ImageByteFormat.png;
    if (Platform.isWindows) {
      imgFormat = ui.ImageByteFormat.rawRgba;
    }

    ByteData? imgBytes = await image.toByteData(format: imgFormat);
    if (imgBytes == null) {
      return false;
    }

    Uint8List? data = imgBytes.buffer.asUint8List();
    _cache = CursorData(
      peerId: this.id,
      id: id,
      image: Platform.isWindows ? img2.Image.fromBytes(w, h, data) : null,
      scale: 1.0,
      data: data,
      hotx: 0,
      hoty: 0,
      // hotx: _hotx,
      // hoty: _hoty,
      width: w,
      height: h,
    );
    _cacheMap[id] = _cache!;
    return true;
  }

  updateCursorId(Map<String, dynamic> evt) async {
    final id = int.parse(evt['id']);
    _cache = _cacheMap[id];
    final tmp = _images[id];
    if (tmp != null) {
      _image = tmp.item1;
      _hotx = tmp.item2;
      _hoty = tmp.item3;
      notifyListeners();
    } else {
      debugPrint(
          'WARNING: updateCursorId $id, cache is ${_cache == null ? "null" : "not null"}. without notifyListeners()');
    }
  }

  /// Update the cursor position.
  updateCursorPosition(Map<String, dynamic> evt, String id) async {
    gotMouseControl = false;
    _lastPeerMouse = DateTime.now();
    _x = double.parse(evt['x']);
    _y = double.parse(evt['y']);
    try {
      RemoteCursorMovedState.find(id).value = true;
    } catch (e) {
      //
    }
    notifyListeners();
  }

  updateDisplayOrigin(double x, double y) {
    _displayOriginX = x;
    _displayOriginY = y;
    _x = x + 1;
    _y = y + 1;
    parent.target?.inputModel.moveMouse(x, y);
    parent.target?.canvasModel.resetOffset();
    notifyListeners();
  }

  updateDisplayOriginWithCursor(
      double x, double y, double xCursor, double yCursor) {
    _displayOriginX = x;
    _displayOriginY = y;
    _x = xCursor;
    _y = yCursor;
    parent.target?.inputModel.moveMouse(x, y);
    notifyListeners();
  }

  clear() {
    _x = -10000;
    _x = -10000;
    _image = null;
    _images.clear();

    _clearCache();
    _cache = null;
    _cacheMap.clear();
  }

  _clearCache() {
    final keys = {...cachedKeys};
    for (var k in keys) {
      customCursorController.freeCache(k);
    }
  }

  Uint8List? cachedForbidmemoryCursorData;
  void updateForbiddenCursorBuffer() {
    cachedForbidmemoryCursorData ??= base64Decode(
        'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAAXNSR0IB2cksfwAAAAlwSFlzAAALEwAACxMBAJqcGAAAAkZQTFRFAAAA2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4G2B4GWAwCAAAAAAAA2B4GAAAAMTExAAAAAAAA2B4G2B4G2B4GAAAAmZmZkZGRAQEBAAAA2B4G2B4G2B4G////oKCgAwMDag8D2B4G2B4G2B4Gra2tBgYGbg8D2B4G2B4Gubm5CQkJTwsCVgwC2B4GxcXFDg4OAAAAAAAA2B4G2B4Gz8/PFBQUAAAAAAAA2B4G2B4G2B4G2B4G2B4G2B4G2B4GDgIA2NjYGxsbAAAAAAAA2B4GFwMB4eHhIyMjAAAAAAAA2B4G6OjoLCwsAAAAAAAA2B4G2B4G2B4G2B4G2B4GCQEA4ODgv7+/iYmJY2NjAgICAAAA9PT0Ojo6AAAAAAAAAAAA+/v7SkpKhYWFr6+vAAAAAAAA8/PzOTk5ERER9fX1KCgoAAAAgYGBKioqAAAAAAAApqamlpaWAAAAAAAAAAAAAAAAAAAAAAAALi4u/v7+GRkZAAAAAAAAAAAAAAAAAAAAfn5+AAAAAAAAV1dXkJCQAAAAAAAAAQEBAAAAAAAAAAAA7Hz6BAAAAMJ0Uk5TAAIWEwEynNz6//fVkCAatP2fDUHs6cDD8d0mPfT5fiEskiIR584A0gejr3AZ+P4plfALf5ZiTL85a4ziD6697fzN3UYE4v/4TwrNHuT///tdRKZh///+1U/ZBv///yjb///eAVL//50Cocv//6oFBbPvpGZCbfT//7cIhv///8INM///zBEcWYSZmO7//////1P////ts/////8vBv//////gv//R/z///QQz9sevP///2waXhNO/+fc//8mev/5gAe2r90MAAAByUlEQVR4nGNggANGJmYWBpyAlY2dg5OTi5uHF6s0H78AJxRwCAphyguLgKRExcQlQLSkFLq8tAwnp6ycPNABjAqKQKNElVDllVU4OVVhVquJA81Q10BRoAkUUYbJa4Edoo0sr6PLqaePLG/AyWlohKTAmJPTBFnelAFoixmSAnNOTgsUeQZLTk4rJAXWnJw2EHlbiDyDPCenHZICe04HFrh+RydnBgYWPU5uJAWinJwucPNd3dw9GDw5Ob2QFHBzcnrD7ffx9fMPCOTkDEINhmC4+3x8Q0LDwlEDIoKTMzIKKg9SEBIdE8sZh6SAJZ6Tkx0qD1YQkpCYlIwclCng0AXLQxSEpKalZyCryATKZwkhKQjJzsnNQ1KQXwBUUVhUXBJYWgZREFJeUVmFpMKlWg+anmqgCkJq6+obkG1pLEBTENLU3NKKrIKhrb2js8u4G6Kgpze0r3/CRAZMAHbkpJDJU6ZMmTqtFbuC6TNmhsyaMnsOFlmwgrnzpsxfELJwEXZ5Bp/FS3yWLlsesmLlKuwKVk9Ys5Zh3foN0zduwq5g85atDAzbpqSGbN9RhV0FGOzctWH3lD14FOzdt3H/gQw8Cg4u2gQPAwBYDXXdIH+wqAAAAABJRU5ErkJggg==');
  }

  img2.Image? defaultCursorImage = img2.decodePng(base64Decode(
      'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAFmSURBVFiF7dWxSlxREMbx34QFDRowYBchZSxSCWlMCOwD5FGEFHap06UI7KPsAyyEEIQFqxRaCqYTsqCJFsKkuAeRXb17wrqV918dztw55zszc2fo6Oh47MR/e3zO1/iAHWmznHKGQwx9ip/LEbCfazbsoY8j/JLOhcC6sCW9wsjEwJf483AC9nPNc1+lFRwI13d+l3rYFS799rFGxJMqARv2pBXh+72XQ7gWvklPS7TmMl9Ak/M+DqrENvxAv/guKKApuKPWl0/TROK4+LbSqzhuB+OZ3fRSeFPWY+Fkyn56Y29hfgTSpnQ+s98cvorVey66uPlNFxKwZOYLCGfCs5n9NMYVrsp6mvXSoFqpqYFDvMBkStgJJe93dZOwVXxbqUnBENulydSReqUrDhcX0PT2EXarBYS3GNXMhboinBgIl9K71kg0L3+PvyYGdVpruT2MwrF0iotiXfIwus0Dj+OOjo6Of+e7ab74RkpgAAAAAElFTkSuQmCC'));
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
      if ((evt['speed'] as String).isNotEmpty) _data.speed = evt['speed'];
      if ((evt['fps'] as String).isNotEmpty) _data.fps = evt['fps'];
      if ((evt['delay'] as String).isNotEmpty) _data.delay = evt['delay'];
      if ((evt['target_bitrate'] as String).isNotEmpty) {
        _data.targetBitrate = evt['target_bitrate'];
      }
      if ((evt['codec_format'] as String).isNotEmpty) {
        _data.codecFormat = evt['codec_format'];
      }
      notifyListeners();
    } catch (e) {
      //
    }
  }
}

class RecordingModel with ChangeNotifier {
  WeakReference<FFI> parent;
  RecordingModel(this.parent);
  bool _start = false;
  get start => _start;

  onSwitchDisplay() {
    if (isIOS || !_start) return;
    var id = parent.target?.id;
    int? width = parent.target?.canvasModel.getDisplayWidth();
    int? height = parent.target?.canvasModel.getDisplayHeight();
    if (id == null || width == null || height == null) return;
    bind.sessionRecordScreen(id: id, start: true, width: width, height: height);
  }

  toggle() {
    if (isIOS) return;
    var id = parent.target?.id;
    if (id == null) return;
    _start = !_start;
    notifyListeners();
    if (_start) {
      bind.sessionRefresh(id: id);
    } else {
      bind.sessionRecordScreen(id: id, start: false, width: 0, height: 0);
    }
  }

  onClose() {
    if (isIOS) return;
    var id = parent.target?.id;
    if (id == null) return;
    _start = false;
    bind.sessionRecordScreen(id: id, start: false, width: 0, height: 0);
  }
}

enum ConnType { defaultConn, fileTransfer, portForward, rdp }

/// Flutter state manager and data communication with the Rust core.
class FFI {
  var id = '';
  var version = '';
  var connType = ConnType.defaultConn;

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
  late final RecordingModel recordingModel; // recording
  late final InputModel inputModel; // session

  FFI() {
    imageModel = ImageModel(WeakReference(this));
    ffiModel = FfiModel(WeakReference(this));
    cursorModel = CursorModel(WeakReference(this));
    canvasModel = CanvasModel(WeakReference(this));
    serverModel = ServerModel(WeakReference(this));
    chatModel = ChatModel(WeakReference(this));
    fileModel = FileModel(WeakReference(this));
    abModel = AbModel(WeakReference(this));
    userModel = UserModel(WeakReference(this));
    qualityMonitorModel = QualityMonitorModel(WeakReference(this));
    recordingModel = RecordingModel(WeakReference(this));
    inputModel = InputModel(WeakReference(this));
  }

  /// Start with the given [id]. Only transfer file if [isFileTransfer], only port forward if [isPortForward].
  void start(String id,
      {bool isFileTransfer = false, bool isPortForward = false}) {
    assert(!(isFileTransfer && isPortForward), 'more than one connect type');
    if (isFileTransfer) {
      connType = ConnType.fileTransfer;
      id = 'ft_$id';
    } else if (isPortForward) {
      connType = ConnType.portForward;
      id = 'pf_$id';
    } else {
      chatModel.resetClientMode();
      canvasModel.id = id;
      imageModel.id = id;
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
            await cb(event);
          } catch (e) {
            debugPrint('json.decode fail1(): $e, ${message.field0}');
          }
        } else if (message is Rgba) {
          imageModel.onRgba(message.field0);
        }
      }
    }();
    // every instance will bind a stream
    this.id = id;
    if (isFileTransfer) {
      fileModel.initFileFetcher();
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
      await setCanvasConfig(id, cursorModel.x, cursorModel.y, canvasModel.x,
          canvasModel.y, canvasModel.scale, ffiModel.pi.currentDisplay);
    }
    bind.sessionClose(id: id);
    id = '';
    imageModel.update(null);
    cursorModel.clear();
    ffiModel.clear();
    canvasModel.clear();
    inputModel.resetModifiers();
    debugPrint('model $id closed');
  }

  void setMethodCallHandler(FMethod callback) {
    platformFFI.setMethodCallHandler(callback);
  }

  Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return await platformFFI.invokeMethod(method, arguments);
  }
}

class Display {
  double x = 0;
  double y = 0;
  int width = 0;
  int height = 0;

  Display() {
    width = (isDesktop || isWebDesktop)
        ? kDesktopDefaultDisplayWidth
        : kMobileDefaultDisplayWidth;
    height = (isDesktop || isWebDesktop)
        ? kDesktopDefaultDisplayHeight
        : kMobileDefaultDisplayHeight;
  }
}

class PeerInfo {
  String version = '';
  String username = '';
  String hostname = '';
  String platform = '';
  bool sasEnabled = false;
  int currentDisplay = 0;
  List<Display> displays = [];
}

const canvasKey = 'canvas';

Future<void> setCanvasConfig(String id, double xCursor, double yCursor,
    double xCanvas, double yCanvas, double scale, int currentDisplay) async {
  final p = <String, dynamic>{};
  p['xCursor'] = xCursor;
  p['yCursor'] = yCursor;
  p['xCanvas'] = xCanvas;
  p['yCanvas'] = yCanvas;
  p['scale'] = scale;
  p['currentDisplay'] = currentDisplay;
  await bind.sessionSetFlutterConfig(id: id, k: canvasKey, v: jsonEncode(p));
}

Future<Map<String, dynamic>?> getCanvasConfig(String id) async {
  if (!isWebDesktop) return null;
  var p = await bind.sessionGetFlutterConfig(id: id, k: canvasKey);
  if (p == null || p.isEmpty) return null;
  try {
    Map<String, dynamic> m = json.decode(p);
    return m;
  } catch (e) {
    return null;
  }
}

void removePreference(String id) async {
  await bind.sessionSetFlutterConfig(id: id, k: canvasKey, v: '');
}

Future<void> initializeCursorAndCanvas(FFI ffi) async {
  var p = await getCanvasConfig(ffi.id);
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
