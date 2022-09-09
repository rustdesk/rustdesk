import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:wakelock/wakelock.dart';
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';

// import 'package:window_manager/window_manager.dart';

import '../widgets/remote_menubar.dart';
import '../../common.dart';
import '../../mobile/widgets/dialog.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../common/shared_state.dart';

final initText = '\1' * 1024;

class RemotePage extends StatefulWidget {
  const RemotePage({
    Key? key,
    required this.id,
    required this.tabBarHeight,
  }) : super(key: key);

  final String id;
  final double tabBarHeight;

  @override
  State<RemotePage> createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage>
    with AutomaticKeepAliveClientMixin {
  Timer? _timer;
  String _value = '';
  String keyboardMode = "legacy";
  final _cursorOverImage = false.obs;
  late RxBool _showRemoteCursor;
  late RxBool _remoteCursorMoved;
  late RxBool _keyboardEnabled;

  final FocusNode _mobileFocusNode = FocusNode();
  final FocusNode _physicalFocusNode = FocusNode();
  var _isPhysicalMouse = false;
  var _imageFocused = false;

  late FFI _ffi;

  void _updateTabBarHeight() {
    _ffi.canvasModel.tabBarHeight = widget.tabBarHeight;
  }

  void _initStates(String id) {
    PrivacyModeState.init(id);
    BlockInputState.init(id);
    CurrentDisplayState.init(id);
    KeyboardEnabledState.init(id);
    ShowRemoteCursorState.init(id);
    RemoteCursorMovedState.init(id);
    _showRemoteCursor = ShowRemoteCursorState.find(id);
    _keyboardEnabled = KeyboardEnabledState.find(id);
    _remoteCursorMoved = RemoteCursorMovedState.find(id);
  }

  void _removeStates(String id) {
    PrivacyModeState.delete(id);
    BlockInputState.delete(id);
    CurrentDisplayState.delete(id);
    ShowRemoteCursorState.delete(id);
    KeyboardEnabledState.delete(id);
    RemoteCursorMovedState.delete(id);
  }

  @override
  void initState() {
    super.initState();
    _initStates(widget.id);
    _ffi = FFI();
    _updateTabBarHeight();
    Get.put(_ffi, tag: widget.id);
    _ffi.connect(widget.id, tabBarHeight: super.widget.tabBarHeight);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    if (!Platform.isLinux) {
      Wakelock.enable();
    }
    _physicalFocusNode.requestFocus();
    _ffi.ffiModel.updateEventListener(widget.id);
    _ffi.listenToMouse(true);
    _ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
    // WindowManager.instance.addListener(this);
    _showRemoteCursor.value = bind.sessionGetToggleOptionSync(
        id: widget.id, arg: 'show-remote-cursor');
  }

  @override
  void dispose() {
    debugPrint("REMOTE PAGE dispose ${widget.id}");
    _ffi.dialogManager.hideMobileActionsOverlay();
    _ffi.listenToMouse(false);
    _mobileFocusNode.dispose();
    _physicalFocusNode.dispose();
    _ffi.close();
    _timer?.cancel();
    _ffi.dialogManager.dismissAll();
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    if (!Platform.isLinux) {
      Wakelock.disable();
    }
    // WindowManager.instance.removeListener(this);
    Get.delete<FFI>(tag: widget.id);
    super.dispose();
    _removeStates(widget.id);
  }

  void resetTool() {
    _ffi.resetModifiers();
  }

  // handle mobile virtual keyboard
  void handleInput(String newValue) {
    var oldValue = _value;
    _value = newValue;
    if (isIOS) {
      var i = newValue.length - 1;
      for (; i >= 0 && newValue[i] != '\1'; --i) {}
      var j = oldValue.length - 1;
      for (; j >= 0 && oldValue[j] != '\1'; --j) {}
      if (i < j) j = i;
      newValue = newValue.substring(j + 1);
      oldValue = oldValue.substring(j + 1);
      var common = 0;
      for (;
          common < oldValue.length &&
              common < newValue.length &&
              newValue[common] == oldValue[common];
          ++common) {}
      for (i = 0; i < oldValue.length - common; ++i) {
        _ffi.inputKey('VK_BACK');
      }
      if (newValue.length > common) {
        var s = newValue.substring(common);
        if (s.length > 1) {
          bind.sessionInputString(id: widget.id, value: s);
        } else {
          inputChar(s);
        }
      }
      return;
    }
    if (oldValue.isNotEmpty &&
        newValue.isNotEmpty &&
        oldValue[0] == '\1' &&
        newValue[0] != '\1') {
      // clipboard
      oldValue = '';
    }
    if (newValue.length == oldValue.length) {
      // ?
    } else if (newValue.length < oldValue.length) {
      const char = 'VK_BACK';
      _ffi.inputKey(char);
    } else {
      final content = newValue.substring(oldValue.length);
      if (content.length > 1) {
        if (oldValue != '' &&
            content.length == 2 &&
            (content == '""' ||
                content == '()' ||
                content == '[]' ||
                content == '<>' ||
                content == "{}" ||
                content == '”“' ||
                content == '《》' ||
                content == '（）' ||
                content == '【】')) {
          // can not only input content[0], because when input ], [ are also auo insert, which cause ] never be input
          bind.sessionInputString(id: widget.id, value: content);
          return;
        }
        bind.sessionInputString(id: widget.id, value: content);
      } else {
        inputChar(content);
      }
    }
  }

  void inputChar(String char) {
    if (char == '\n') {
      char = 'VK_RETURN';
    } else if (char == ' ') {
      char = 'VK_SPACE';
    }
    _ffi.inputKey(char);
  }

  void sendRawKey(RawKeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = _logicalKeyMap[e.logicalKey.keyId] ??
        _physicalKeyMap[e.physicalKey.usbHidUsage] ??
        e.logicalKey.keyLabel;
    _ffi.inputKey(label, down: down, press: press ?? false);
  }

  Widget buildBody(BuildContext context) {
    return Scaffold(
        backgroundColor: MyTheme.color(context).bg,
        body: Overlay(
          initialEntries: [
            OverlayEntry(builder: (context) {
              _ffi.chatModel.setOverlayState(Overlay.of(context));
              _ffi.dialogManager.setOverlayState(Overlay.of(context));
              return Container(
                  color: Colors.black,
                  child: getRawPointerAndKeyBody(getBodyForDesktop(context)));
            })
          ],
        ));
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    _updateTabBarHeight();
    return WillPopScope(
        onWillPop: () async {
          clientClose(_ffi.dialogManager);
          return false;
        },
        child: MultiProvider(providers: [
          ChangeNotifierProvider.value(value: _ffi.ffiModel),
          ChangeNotifierProvider.value(value: _ffi.imageModel),
          ChangeNotifierProvider.value(value: _ffi.cursorModel),
          ChangeNotifierProvider.value(value: _ffi.canvasModel),
        ], child: buildBody(context)));
  }

  KeyEventResult handleRawKeyEvent(FocusNode data, RawKeyEvent e) {
    bind.sessionGetKeyboardName(id: widget.id).then((result) {
      setState(() {
        keyboardMode = result.toString();
      });
    });

    if (keyboardMode == 'map') {
      mapKeyboardMode(e);
    } else if (keyboardMode == 'translate') {
      legacyKeyboardMode(e);
    } else {
      legacyKeyboardMode(e);
    }

    return KeyEventResult.handled;
  }

  void mapKeyboardMode(RawKeyEvent e) {
    int scanCode;
    int keyCode;
    bool down;

    if (e.data is RawKeyEventDataMacOs) {
      RawKeyEventDataMacOs newData = e.data as RawKeyEventDataMacOs;
      scanCode = newData.keyCode;
      keyCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataWindows) {
      RawKeyEventDataWindows newData = e.data as RawKeyEventDataWindows;
      scanCode = newData.scanCode;
      keyCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataLinux) {
      RawKeyEventDataLinux newData = e.data as RawKeyEventDataLinux;
      scanCode = newData.scanCode;
      keyCode = newData.keyCode;
    } else {
      scanCode = -1;
      keyCode = -1;
    }

    if (e is RawKeyDownEvent) {
      down = true;
    } else {
      down = false;
    }

    _ffi.inputRawKey(e.character ?? "", keyCode, scanCode, down);
  }

  void legacyKeyboardMode(RawKeyEvent e) {
    final key = e.logicalKey;
    if (e is RawKeyDownEvent) {
      if (e.repeat) {
        sendRawKey(e, press: true);
      } else {
        if (e.isAltPressed && !_ffi.alt) {
          _ffi.alt = true;
        } else if (e.isControlPressed && !_ffi.ctrl) {
          _ffi.ctrl = true;
        } else if (e.isShiftPressed && !_ffi.shift) {
          _ffi.shift = true;
        } else if (e.isMetaPressed && !_ffi.command) {
          _ffi.command = true;
        }
        sendRawKey(e, down: true);
      }
    }
    if (e is RawKeyUpEvent) {
      if (key == LogicalKeyboardKey.altLeft ||
          key == LogicalKeyboardKey.altRight) {
        _ffi.alt = false;
      } else if (key == LogicalKeyboardKey.controlLeft ||
          key == LogicalKeyboardKey.controlRight) {
        _ffi.ctrl = false;
      } else if (key == LogicalKeyboardKey.shiftRight ||
          key == LogicalKeyboardKey.shiftLeft) {
        _ffi.shift = false;
      } else if (key == LogicalKeyboardKey.metaLeft ||
          key == LogicalKeyboardKey.metaRight ||
          key == LogicalKeyboardKey.superKey) {
        _ffi.command = false;
      }
      sendRawKey(e);
    }
  }

  Widget getRawPointerAndKeyBody(Widget child) {
    return FocusScope(
        autofocus: true,
        child: Focus(
            autofocus: true,
            canRequestFocus: true,
            focusNode: _physicalFocusNode,
            onFocusChange: (bool v) {
              _imageFocused = v;
            },
            onKey: handleRawKeyEvent,
            child: child));
  }

  /// touchMode only:
  ///   LongPress -> right click
  ///   OneFingerPan -> start/end -> left down start/end
  ///   onDoubleTapDown -> move to
  ///   onLongPressDown => move to
  ///
  /// mouseMode only:
  ///   DoubleFiner -> right click
  ///   HoldDrag -> left drag
  void _onPointHoverImage(PointerHoverEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (!_isPhysicalMouse) {
      setState(() {
        _isPhysicalMouse = true;
      });
    }
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousemove'),
          tabBarHeight: widget.tabBarHeight);
    }
  }

  void _onPointDownImage(PointerDownEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) {
      if (_isPhysicalMouse) {
        setState(() {
          _isPhysicalMouse = false;
        });
      }
    }
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousedown'),
          tabBarHeight: widget.tabBarHeight);
    }
  }

  void _onPointUpImage(PointerUpEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mouseup'),
          tabBarHeight: widget.tabBarHeight);
    }
  }

  void _onPointMoveImage(PointerMoveEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousemove'),
          tabBarHeight: widget.tabBarHeight);
    }
  }

  void _onPointerSignalImage(PointerSignalEvent e) {
    if (e is PointerScrollEvent) {
      var dx = e.scrollDelta.dx.toInt();
      var dy = e.scrollDelta.dy.toInt();
      if (dx > 0) {
        dx = -1;
      } else if (dx < 0) {
        dx = 1;
      }
      if (dy > 0) {
        dy = -1;
      } else if (dy < 0) {
        dy = 1;
      }
      bind.sessionSendMouse(
          id: widget.id, msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }

  void enterView(PointerEnterEvent evt) {
    if (!_imageFocused) {
      _physicalFocusNode.requestFocus();
    }
    _cursorOverImage.value = true;
    _ffi.enterOrLeave(true);
  }

  void leaveView(PointerExitEvent evt) {
    _cursorOverImage.value = false;
    _ffi.enterOrLeave(false);
  }

  Widget _buildImageListener(Widget child) {
    return Listener(
        onPointerHover: _onPointHoverImage,
        onPointerDown: _onPointDownImage,
        onPointerUp: _onPointUpImage,
        onPointerMove: _onPointMoveImage,
        onPointerSignal: _onPointerSignalImage,
        child:
            MouseRegion(onEnter: enterView, onExit: leaveView, child: child));
  }

  Widget getBodyForDesktop(BuildContext context) {
    var paints = <Widget>[
      MouseRegion(onEnter: (evt) {
        bind.hostStopSystemKeyPropagate(stopped: false);
      }, onExit: (evt) {
        bind.hostStopSystemKeyPropagate(stopped: true);
      }, child: LayoutBuilder(builder: (context, constraints) {
        Future.delayed(Duration.zero, () {
          Provider.of<CanvasModel>(context, listen: false).updateViewStyle();
        });
        return ImagePaint(
          id: widget.id,
          cursorOverImage: _cursorOverImage,
          keyboardEnabled: _keyboardEnabled,
          remoteCursorMoved: _remoteCursorMoved,
          listenerBuilder: _buildImageListener,
        );
      }))
    ];

    paints.add(Obx(() => Visibility(
        visible: _showRemoteCursor.isTrue && _remoteCursorMoved.isTrue,
        child: CursorPaint(
          id: widget.id,
        ))));
    paints.add(QualityMonitor(_ffi.qualityMonitorModel));
    paints.add(RemoteMenubar(
      id: widget.id,
      ffi: _ffi,
    ));
    return Stack(
      children: paints,
    );
  }

  int lastMouseDownButtons = 0;

  Map<String, dynamic> getEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};
    out['type'] = type;
    out['x'] = evt.position.dx;
    out['y'] = evt.position.dy;
    if (_ffi.alt) out['alt'] = 'true';
    if (_ffi.shift) out['shift'] = 'true';
    if (_ffi.ctrl) out['ctrl'] = 'true';
    if (_ffi.command) out['command'] = 'true';
    out['buttons'] = evt
        .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
    if (evt.buttons != 0) {
      lastMouseDownButtons = evt.buttons;
    } else {
      out['buttons'] = lastMouseDownButtons;
    }
    return out;
  }

  @override
  void onWindowEvent(String eventName) {
    print("window event: $eventName");
    switch (eventName) {
      case 'resize':
        _ffi.canvasModel.updateViewStyle();
        break;
      case 'maximize':
        Future.delayed(const Duration(milliseconds: 100), () {
          _ffi.canvasModel.updateViewStyle();
        });
        break;
    }
  }

  @override
  bool get wantKeepAlive => true;
}

class ImagePaint extends StatelessWidget {
  final String id;
  final Rx<bool> cursorOverImage;
  final Rx<bool> keyboardEnabled;
  final Rx<bool> remoteCursorMoved;
  final Widget Function(Widget)? listenerBuilder;
  final ScrollController _horizontal = ScrollController();
  final ScrollController _vertical = ScrollController();

  ImagePaint(
      {Key? key,
      required this.id,
      required this.cursorOverImage,
      required this.keyboardEnabled,
      required this.remoteCursorMoved,
      this.listenerBuilder})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    var c = Provider.of<CanvasModel>(context);
    final s = c.scale;

    mouseRegion({child}) => Obx(() => MouseRegion(
        cursor: (cursorOverImage.isTrue && keyboardEnabled.isTrue)
            ? (remoteCursorMoved.isTrue
                ? SystemMouseCursors.none
                : _buildCustomCursorLinux(context, s))
            : MouseCursor.defer,
        onHover: (evt) {},
        child: child));

    if (c.scrollStyle == ScrollStyle.scrollbar) {
      final imageWidget = SizedBox(
          width: c.getDisplayWidth() * s,
          height: c.getDisplayHeight() * s,
          child: CustomPaint(
            painter: ImagePainter(image: m.image, x: 0, y: 0, scale: s),
          ));

      return Center(
        child: NotificationListener<ScrollNotification>(
          onNotification: (notification) {
            final percentX = _horizontal.position.extentBefore /
                (_horizontal.position.extentBefore +
                    _horizontal.position.extentInside +
                    _horizontal.position.extentAfter);
            final percentY = _vertical.position.extentBefore /
                (_vertical.position.extentBefore +
                    _vertical.position.extentInside +
                    _vertical.position.extentAfter);
            c.setScrollPercent(percentX, percentY);
            return false;
          },
          child: mouseRegion(
              child: _buildCrossScrollbar(_buildListener(imageWidget))),
        ),
      );
    } else {
      final imageWidget = SizedBox(
          width: c.size.width,
          height: c.size.height,
          child: CustomPaint(
            painter:
                ImagePainter(image: m.image, x: c.x / s, y: c.y / s, scale: s),
          ));
      return mouseRegion(child: _buildListener(imageWidget));
    }
  }

  MouseCursor _buildCustomCursorLinux(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cacheLinux = cursor.cacheLinux;
    if (cacheLinux == null) {
      return MouseCursor.defer;
    } else {
      final key = cacheLinux.key(scale);
      cursor.addKeyLinux(key);
      // debugPrint(
      //     'REMOVE ME ================================= linux curor key: $key');
      return FlutterCustomMemoryImageCursor(
        pixbuf: cacheLinux.data,
        key: key,
        hotx: cacheLinux.hotx,
        hoty: cacheLinux.hoty,
        imageWidth: (cacheLinux.width * scale).toInt(),
        imageHeight: (cacheLinux.height * scale).toInt(),
      );
    }
  }

  Widget _buildCrossScrollbar(Widget child) {
    final physicsVertical =
        cursorOverImage.value ? const NeverScrollableScrollPhysics() : null;
    final physicsHorizontal =
        cursorOverImage.value ? const NeverScrollableScrollPhysics() : null;
    return Scrollbar(
        controller: _vertical,
        thumbVisibility: true,
        trackVisibility: true,
        child: Scrollbar(
          controller: _horizontal,
          thumbVisibility: true,
          trackVisibility: true,
          notificationPredicate: (notif) => notif.depth == 1,
          child: SingleChildScrollView(
            controller: _vertical,
            physics: physicsVertical,
            child: SingleChildScrollView(
              controller: _horizontal,
              scrollDirection: Axis.horizontal,
              physics: physicsHorizontal,
              child: child,
            ),
          ),
        ));
  }

  Widget _buildListener(Widget child) {
    if (listenerBuilder != null) {
      return listenerBuilder!(child);
    } else {
      return child;
    }
  }
}

class CursorPaint extends StatelessWidget {
  final String id;

  const CursorPaint({Key? key, required this.id}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    // final adjust = m.adjustForKeyboard();
    return CustomPaint(
      painter: ImagePainter(
          image: m.image,
          x: m.x - m.hotx + c.x / c.scale,
          y: m.y - m.hoty + c.y / c.scale,
          scale: c.scale),
    );
  }
}

class ImagePainter extends CustomPainter {
  ImagePainter({
    required this.image,
    required this.x,
    required this.y,
    required this.scale,
  });

  ui.Image? image;
  double x;
  double y;
  double scale;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    if (x.isNaN || y.isNaN) return;
    canvas.scale(scale, scale);
    // https://github.com/flutter/flutter/issues/76187#issuecomment-784628161
    // https://api.flutter-io.cn/flutter/dart-ui/FilterQuality.html
    var paint = Paint();
    paint.filterQuality = FilterQuality.medium;
    if (scale > 10.00000) {
      paint.filterQuality = FilterQuality.high;
    }
    canvas.drawImage(image!, Offset(x, y), paint);
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}

class QualityMonitor extends StatelessWidget {
  final QualityMonitorModel qualityMonitorModel;
  QualityMonitor(this.qualityMonitorModel);

  @override
  Widget build(BuildContext context) => ChangeNotifierProvider.value(
      value: qualityMonitorModel,
      child: Consumer<QualityMonitorModel>(
          builder: (context, qualityMonitorModel, child) => Positioned(
              top: 10,
              right: 10,
              child: qualityMonitorModel.show
                  ? Container(
                      padding: const EdgeInsets.all(8),
                      color: MyTheme.canvasColor.withAlpha(120),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            "Speed: ${qualityMonitorModel.data.speed ?? ''}",
                            style: const TextStyle(color: MyTheme.grayBg),
                          ),
                          Text(
                            "FPS: ${qualityMonitorModel.data.fps ?? ''}",
                            style: const TextStyle(color: MyTheme.grayBg),
                          ),
                          Text(
                            "Delay: ${qualityMonitorModel.data.delay ?? ''} ms",
                            style: const TextStyle(color: MyTheme.grayBg),
                          ),
                          Text(
                            "Target Bitrate: ${qualityMonitorModel.data.targetBitrate ?? ''}kb",
                            style: const TextStyle(color: MyTheme.grayBg),
                          ),
                          Text(
                            "Codec: ${qualityMonitorModel.data.codecFormat ?? ''}",
                            style: const TextStyle(color: MyTheme.grayBg),
                          ),
                        ],
                      ),
                    )
                  : const SizedBox.shrink())));
}

void sendPrompt(String id, bool isMac, String key) {
  FFI _ffi = ffi(id);
  final old = isMac ? _ffi.command : _ffi.ctrl;
  if (isMac) {
    _ffi.command = true;
  } else {
    _ffi.ctrl = true;
  }
  _ffi.inputKey(key);
  if (isMac) {
    _ffi.command = old;
  } else {
    _ffi.ctrl = old;
  }
}

/// flutter/packages/flutter/lib/src/services/keyboard_key.dart -> _keyLabels
/// see [LogicalKeyboardKey.keyLabel]
const Map<int, String> _logicalKeyMap = <int, String>{
  0x00000000020: 'VK_SPACE',
  0x00000000022: 'VK_QUOTE',
  0x0000000002c: 'VK_COMMA',
  0x0000000002d: 'VK_MINUS',
  0x0000000002f: 'VK_SLASH',
  0x00000000030: 'VK_0',
  0x00000000031: 'VK_1',
  0x00000000032: 'VK_2',
  0x00000000033: 'VK_3',
  0x00000000034: 'VK_4',
  0x00000000035: 'VK_5',
  0x00000000036: 'VK_6',
  0x00000000037: 'VK_7',
  0x00000000038: 'VK_8',
  0x00000000039: 'VK_9',
  0x0000000003b: 'VK_SEMICOLON',
  0x0000000003d: 'VK_PLUS', // it is =
  0x0000000005b: 'VK_LBRACKET',
  0x0000000005c: 'VK_BACKSLASH',
  0x0000000005d: 'VK_RBRACKET',
  0x00000000061: 'VK_A',
  0x00000000062: 'VK_B',
  0x00000000063: 'VK_C',
  0x00000000064: 'VK_D',
  0x00000000065: 'VK_E',
  0x00000000066: 'VK_F',
  0x00000000067: 'VK_G',
  0x00000000068: 'VK_H',
  0x00000000069: 'VK_I',
  0x0000000006a: 'VK_J',
  0x0000000006b: 'VK_K',
  0x0000000006c: 'VK_L',
  0x0000000006d: 'VK_M',
  0x0000000006e: 'VK_N',
  0x0000000006f: 'VK_O',
  0x00000000070: 'VK_P',
  0x00000000071: 'VK_Q',
  0x00000000072: 'VK_R',
  0x00000000073: 'VK_S',
  0x00000000074: 'VK_T',
  0x00000000075: 'VK_U',
  0x00000000076: 'VK_V',
  0x00000000077: 'VK_W',
  0x00000000078: 'VK_X',
  0x00000000079: 'VK_Y',
  0x0000000007a: 'VK_Z',
  0x00100000008: 'VK_BACK',
  0x00100000009: 'VK_TAB',
  0x0010000000d: 'VK_ENTER',
  0x0010000001b: 'VK_ESCAPE',
  0x0010000007f: 'VK_DELETE',
  0x00100000104: 'VK_CAPITAL',
  0x00100000301: 'VK_DOWN',
  0x00100000302: 'VK_LEFT',
  0x00100000303: 'VK_RIGHT',
  0x00100000304: 'VK_UP',
  0x00100000305: 'VK_END',
  0x00100000306: 'VK_HOME',
  0x00100000307: 'VK_NEXT',
  0x00100000308: 'VK_PRIOR',
  0x00100000401: 'VK_CLEAR',
  0x00100000407: 'VK_INSERT',
  0x00100000504: 'VK_CANCEL',
  0x00100000506: 'VK_EXECUTE',
  0x00100000508: 'VK_HELP',
  0x00100000509: 'VK_PAUSE',
  0x0010000050c: 'VK_SELECT',
  0x00100000608: 'VK_PRINT',
  0x00100000705: 'VK_CONVERT',
  0x00100000706: 'VK_FINAL',
  0x00100000711: 'VK_HANGUL',
  0x00100000712: 'VK_HANJA',
  0x00100000713: 'VK_JUNJA',
  0x00100000718: 'VK_KANA',
  0x00100000719: 'VK_KANJI',
  0x00100000801: 'VK_F1',
  0x00100000802: 'VK_F2',
  0x00100000803: 'VK_F3',
  0x00100000804: 'VK_F4',
  0x00100000805: 'VK_F5',
  0x00100000806: 'VK_F6',
  0x00100000807: 'VK_F7',
  0x00100000808: 'VK_F8',
  0x00100000809: 'VK_F9',
  0x0010000080a: 'VK_F10',
  0x0010000080b: 'VK_F11',
  0x0010000080c: 'VK_F12',
  0x00100000d2b: 'Apps',
  0x00200000002: 'VK_SLEEP',
  0x00200000100: 'VK_CONTROL',
  0x00200000101: 'RControl',
  0x00200000102: 'VK_SHIFT',
  0x00200000103: 'RShift',
  0x00200000104: 'VK_MENU',
  0x00200000105: 'RAlt',
  0x002000001f0: 'VK_CONTROL',
  0x002000001f2: 'VK_SHIFT',
  0x002000001f4: 'VK_MENU',
  0x002000001f6: 'Meta',
  0x0020000022a: 'VK_MULTIPLY',
  0x0020000022b: 'VK_ADD',
  0x0020000022d: 'VK_SUBTRACT',
  0x0020000022e: 'VK_DECIMAL',
  0x0020000022f: 'VK_DIVIDE',
  0x00200000230: 'VK_NUMPAD0',
  0x00200000231: 'VK_NUMPAD1',
  0x00200000232: 'VK_NUMPAD2',
  0x00200000233: 'VK_NUMPAD3',
  0x00200000234: 'VK_NUMPAD4',
  0x00200000235: 'VK_NUMPAD5',
  0x00200000236: 'VK_NUMPAD6',
  0x00200000237: 'VK_NUMPAD7',
  0x00200000238: 'VK_NUMPAD8',
  0x00200000239: 'VK_NUMPAD9',
};

/// flutter/packages/flutter/lib/src/services/keyboard_key.dart -> _debugName
/// see [PhysicalKeyboardKey.debugName] -> _debugName
const Map<int, String> _physicalKeyMap = <int, String>{
  0x00010082: 'VK_SLEEP',
  0x00070004: 'VK_A',
  0x00070005: 'VK_B',
  0x00070006: 'VK_C',
  0x00070007: 'VK_D',
  0x00070008: 'VK_E',
  0x00070009: 'VK_F',
  0x0007000a: 'VK_G',
  0x0007000b: 'VK_H',
  0x0007000c: 'VK_I',
  0x0007000d: 'VK_J',
  0x0007000e: 'VK_K',
  0x0007000f: 'VK_L',
  0x00070010: 'VK_M',
  0x00070011: 'VK_N',
  0x00070012: 'VK_O',
  0x00070013: 'VK_P',
  0x00070014: 'VK_Q',
  0x00070015: 'VK_R',
  0x00070016: 'VK_S',
  0x00070017: 'VK_T',
  0x00070018: 'VK_U',
  0x00070019: 'VK_V',
  0x0007001a: 'VK_W',
  0x0007001b: 'VK_X',
  0x0007001c: 'VK_Y',
  0x0007001d: 'VK_Z',
  0x0007001e: 'VK_1',
  0x0007001f: 'VK_2',
  0x00070020: 'VK_3',
  0x00070021: 'VK_4',
  0x00070022: 'VK_5',
  0x00070023: 'VK_6',
  0x00070024: 'VK_7',
  0x00070025: 'VK_8',
  0x00070026: 'VK_9',
  0x00070027: 'VK_0',
  0x00070028: 'VK_ENTER',
  0x00070029: 'VK_ESCAPE',
  0x0007002a: 'VK_BACK',
  0x0007002b: 'VK_TAB',
  0x0007002c: 'VK_SPACE',
  0x0007002d: 'VK_MINUS',
  0x0007002e: 'VK_PLUS', // it is =
  0x0007002f: 'VK_LBRACKET',
  0x00070030: 'VK_RBRACKET',
  0x00070033: 'VK_SEMICOLON',
  0x00070034: 'VK_QUOTE',
  0x00070036: 'VK_COMMA',
  0x00070038: 'VK_SLASH',
  0x00070039: 'VK_CAPITAL',
  0x0007003a: 'VK_F1',
  0x0007003b: 'VK_F2',
  0x0007003c: 'VK_F3',
  0x0007003d: 'VK_F4',
  0x0007003e: 'VK_F5',
  0x0007003f: 'VK_F6',
  0x00070040: 'VK_F7',
  0x00070041: 'VK_F8',
  0x00070042: 'VK_F9',
  0x00070043: 'VK_F10',
  0x00070044: 'VK_F11',
  0x00070045: 'VK_F12',
  0x00070049: 'VK_INSERT',
  0x0007004a: 'VK_HOME',
  0x0007004b: 'VK_PRIOR', // Page Up
  0x0007004c: 'VK_DELETE',
  0x0007004d: 'VK_END',
  0x0007004e: 'VK_NEXT', // Page Down
  0x0007004f: 'VK_RIGHT',
  0x00070050: 'VK_LEFT',
  0x00070051: 'VK_DOWN',
  0x00070052: 'VK_UP',
  0x00070053: 'Num Lock', // TODO rust not impl
  0x00070054: 'VK_DIVIDE', // numpad
  0x00070055: 'VK_MULTIPLY',
  0x00070056: 'VK_SUBTRACT',
  0x00070057: 'VK_ADD',
  0x00070058: 'VK_ENTER', // num enter
  0x00070059: 'VK_NUMPAD0',
  0x0007005a: 'VK_NUMPAD1',
  0x0007005b: 'VK_NUMPAD2',
  0x0007005c: 'VK_NUMPAD3',
  0x0007005d: 'VK_NUMPAD4',
  0x0007005e: 'VK_NUMPAD5',
  0x0007005f: 'VK_NUMPAD6',
  0x00070060: 'VK_NUMPAD7',
  0x00070061: 'VK_NUMPAD8',
  0x00070062: 'VK_NUMPAD9',
  0x00070063: 'VK_DECIMAL',
  0x00070075: 'VK_HELP',
  0x00070077: 'VK_SELECT',
  0x00070088: 'VK_KANA',
  0x0007008a: 'VK_CONVERT',
  0x000700e0: 'VK_CONTROL',
  0x000700e1: 'VK_SHIFT',
  0x000700e2: 'VK_MENU',
  0x000700e3: 'Meta',
  0x000700e4: 'RControl',
  0x000700e5: 'RShift',
  0x000700e6: 'RAlt',
  0x000700e7: 'RWin',
  0x000c00b1: 'VK_PAUSE',
  0x000c00cd: 'VK_PAUSE',
  0x000c019e: 'LOCK_SCREEN',
  0x000c0208: 'VK_PRINT',
};
