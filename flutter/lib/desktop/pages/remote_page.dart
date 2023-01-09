import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart'
    as custom_cursor_manager;
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:wakelock/wakelock.dart';
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';
import 'package:flutter_improved_scrolling/flutter_improved_scrolling.dart';

import '../../consts.dart';
import '../../common/widgets/overlay.dart';
import '../../common/widgets/remote_input.dart';
import '../../common.dart';
import '../../mobile/widgets/dialog.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../common/shared_state.dart';
import '../widgets/remote_menubar.dart';
import '../widgets/kb_layout_type_chooser.dart';

bool _isCustomCursorInited = false;
final SimpleWrapper<bool> _firstEnterImage = SimpleWrapper(false);

class RemotePage extends StatefulWidget {
  RemotePage({
    Key? key,
    required this.id,
    required this.menubarState,
  }) : super(key: key);

  final String id;
  final MenubarState menubarState;
  final SimpleWrapper<State<RemotePage>?> _lastState = SimpleWrapper(null);

  FFI get ffi => (_lastState.value! as _RemotePageState)._ffi;

  @override
  State<RemotePage> createState() {
    final state = _RemotePageState();
    _lastState.value = state;
    return state;
  }
}

class _RemotePageState extends State<RemotePage>
    with AutomaticKeepAliveClientMixin, MultiWindowListener {
  Timer? _timer;
  String keyboardMode = "legacy";
  bool _isWindowBlur = false;
  final _cursorOverImage = false.obs;
  late RxBool _showRemoteCursor;
  late RxBool _zoomCursor;
  late RxBool _remoteCursorMoved;
  late RxBool _keyboardEnabled;

  final FocusNode _rawKeyFocusNode = FocusNode(debugLabel: "rawkeyFocusNode");

  Function(bool)? _onEnterOrLeaveImage4Menubar;

  late FFI _ffi;

  void _initStates(String id) {
    PrivacyModeState.init(id);
    BlockInputState.init(id);
    CurrentDisplayState.init(id);
    KeyboardEnabledState.init(id);
    ShowRemoteCursorState.init(id);
    RemoteCursorMovedState.init(id);
    final optZoomCursor = 'zoom-cursor';
    PeerBoolOption.init(id, optZoomCursor, () => false);
    _zoomCursor = PeerBoolOption.find(id, optZoomCursor);
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
    Get.put(_ffi, tag: widget.id);
    _ffi.imageModel.addCallbackOnFirstImage((String peerId) {
      showKBLayoutTypeChooserIfNeeded(
          _ffi.ffiModel.pi.platform, _ffi.dialogManager);
    });
    _ffi.start(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    if (!Platform.isLinux) {
      Wakelock.enable();
    }
    _ffi.ffiModel.updateEventListener(widget.id);
    _ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
    // Session option should be set after models.dart/FFI.start
    _showRemoteCursor.value = bind.sessionGetToggleOptionSync(
        id: widget.id, arg: 'show-remote-cursor');
    _zoomCursor.value =
        bind.sessionGetToggleOptionSync(id: widget.id, arg: 'zoom-cursor');
    DesktopMultiWindow.addListener(this);
    // if (!_isCustomCursorInited) {
    //   customCursorController.registerNeedUpdateCursorCallback(
    //       (String? lastKey, String? currentKey) async {
    //     if (_firstEnterImage.value) {
    //       _firstEnterImage.value = false;
    //       return true;
    //     }
    //     return lastKey == null || lastKey != currentKey;
    //   });
    //   _isCustomCursorInited = true;
    // }
  }

  @override
  void onWindowBlur() {
    super.onWindowBlur();
    // On windows, we use `focus` way to handle keyboard better.
    // Now on Linux, there's some rdev issues which will break the input.
    // We disable the `focus` way for non-Windows temporarily.
    if (Platform.isWindows) {
      _isWindowBlur = true;
      // unfocus the primary-focus when the whole window is lost focus,
      // and let OS to handle events instead.
      _rawKeyFocusNode.unfocus();
    }
  }

  @override
  void onWindowFocus() {
    super.onWindowFocus();
    // See [onWindowBlur].
    if (Platform.isWindows) {
      _isWindowBlur = false;
    }
  }

  @override
  void onWindowRestore() {
    super.onWindowRestore();
    // On windows, we use `onWindowRestore` way to handle window restore from
    // a minimized state.
    if (Platform.isWindows) {
      _isWindowBlur = false;
    }
  }

  @override
  void dispose() {
    debugPrint("REMOTE PAGE dispose ${widget.id}");
    // ensure we leave this session, this is a double check
    bind.sessionEnterOrLeave(id: widget.id, enter: false);
    DesktopMultiWindow.removeListener(this);
    _ffi.dialogManager.hideMobileActionsOverlay();
    _ffi.recordingModel.onClose();
    _rawKeyFocusNode.dispose();
    _ffi.close();
    _timer?.cancel();
    _ffi.dialogManager.dismissAll();
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    if (!Platform.isLinux) {
      Wakelock.disable();
    }
    Get.delete<FFI>(tag: widget.id);
    super.dispose();
    _removeStates(widget.id);
  }

  Widget buildBody(BuildContext context) {
    return Scaffold(
        backgroundColor: Theme.of(context).backgroundColor,
        body: Overlay(
          initialEntries: [
            OverlayEntry(builder: (context) {
              _ffi.chatModel.setOverlayState(Overlay.of(context));
              _ffi.dialogManager.setOverlayState(Overlay.of(context));
              return Container(
                  color: Colors.black,
                  child: RawKeyFocusScope(
                      focusNode: _rawKeyFocusNode,
                      onFocusChange: (bool imageFocused) {
                        debugPrint(
                            "onFocusChange(window active:${!_isWindowBlur}) $imageFocused");
                        // See [onWindowBlur].
                        if (Platform.isWindows) {
                          if (_isWindowBlur) {
                            imageFocused = false;
                            Future.delayed(Duration.zero, () {
                              _rawKeyFocusNode.unfocus();
                            });
                          }
                          if (imageFocused) {
                            _ffi.inputModel.enterOrLeave(true);
                          } else {
                            _ffi.inputModel.enterOrLeave(false);
                          }
                        }
                      },
                      inputModel: _ffi.inputModel,
                      child: getBodyForDesktop(context)));
            })
          ],
        ));
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return WillPopScope(
        onWillPop: () async {
          clientClose(widget.id, _ffi.dialogManager);
          return false;
        },
        child: MultiProvider(providers: [
          ChangeNotifierProvider.value(value: _ffi.ffiModel),
          ChangeNotifierProvider.value(value: _ffi.imageModel),
          ChangeNotifierProvider.value(value: _ffi.cursorModel),
          ChangeNotifierProvider.value(value: _ffi.canvasModel),
          ChangeNotifierProvider.value(value: _ffi.recordingModel),
        ], child: buildBody(context)));
  }

  void enterView(PointerEnterEvent evt) {
    _cursorOverImage.value = true;
    _firstEnterImage.value = true;
    if (_onEnterOrLeaveImage4Menubar != null) {
      try {
        _onEnterOrLeaveImage4Menubar!(true);
      } catch (e) {
        //
      }
    }
    // See [onWindowBlur].
    if (!Platform.isWindows) {
      if (!_rawKeyFocusNode.hasFocus) {
        _rawKeyFocusNode.requestFocus();
      }
      bind.sessionEnterOrLeave(id: widget.id, enter: true);
    }
  }

  void leaveView(PointerExitEvent evt) {
    _cursorOverImage.value = false;
    _firstEnterImage.value = false;
    if (_onEnterOrLeaveImage4Menubar != null) {
      try {
        _onEnterOrLeaveImage4Menubar!(false);
      } catch (e) {
        //
      }
    }
    // See [onWindowBlur].
    if (!Platform.isWindows) {
      bind.sessionEnterOrLeave(id: widget.id, enter: false);
    }
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
          zoomCursor: _zoomCursor,
          cursorOverImage: _cursorOverImage,
          keyboardEnabled: _keyboardEnabled,
          remoteCursorMoved: _remoteCursorMoved,
          listenerBuilder: (child) => RawPointerMouseRegion(
            onEnter: enterView,
            onExit: leaveView,
            onPointerDown: (event) {
              // A double check for blur status.
              // Note: If there's an `onPointerDown` event is triggered, `_isWindowBlur` is expected being false.
              // Sometimes the system does not send the necessary focus event to flutter. We should manually
              // handle this inconsistent status by setting `_isWindowBlur` to false. So we can
              // ensure the grab-key thread is running when our users are clicking the remote canvas.
              if (_isWindowBlur) {
                debugPrint(
                    "Unexpected status: onPointerDown is triggered while the remote window is in blur status");
                _isWindowBlur = false;
              }
              if (!_rawKeyFocusNode.hasFocus) {
                _rawKeyFocusNode.requestFocus();
              }
            },
            inputModel: _ffi.inputModel,
            child: child,
          ),
        );
      }))
    ];

    if (!_ffi.canvasModel.cursorEmbedded) {
      paints.add(Obx(() => Offstage(
          offstage: _showRemoteCursor.isFalse || _remoteCursorMoved.isFalse,
          child: CursorPaint(
            id: widget.id,
            zoomCursor: _zoomCursor,
          ))));
    }
    paints.add(QualityMonitor(_ffi.qualityMonitorModel));
    paints.add(RemoteMenubar(
      id: widget.id,
      ffi: _ffi,
      state: widget.menubarState,
      onEnterOrLeaveImageSetter: (func) => _onEnterOrLeaveImage4Menubar = func,
      onEnterOrLeaveImageCleaner: () => _onEnterOrLeaveImage4Menubar = null,
    ));
    return Stack(
      children: paints,
    );
  }

  @override
  bool get wantKeepAlive => true;
}

class ImagePaint extends StatefulWidget {
  final String id;
  final Rx<bool> zoomCursor;
  final Rx<bool> cursorOverImage;
  final Rx<bool> keyboardEnabled;
  final Rx<bool> remoteCursorMoved;
  final Widget Function(Widget)? listenerBuilder;

  ImagePaint(
      {Key? key,
      required this.id,
      required this.zoomCursor,
      required this.cursorOverImage,
      required this.keyboardEnabled,
      required this.remoteCursorMoved,
      this.listenerBuilder})
      : super(key: key);

  @override
  State<StatefulWidget> createState() => _ImagePaintState();
}

class _ImagePaintState extends State<ImagePaint> {
  bool _lastRemoteCursorMoved = false;
  final ScrollController _horizontal = ScrollController();
  final ScrollController _vertical = ScrollController();

  String get id => widget.id;
  Rx<bool> get zoomCursor => widget.zoomCursor;
  Rx<bool> get cursorOverImage => widget.cursorOverImage;
  Rx<bool> get keyboardEnabled => widget.keyboardEnabled;
  Rx<bool> get remoteCursorMoved => widget.remoteCursorMoved;
  Widget Function(Widget)? get listenerBuilder => widget.listenerBuilder;

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    var c = Provider.of<CanvasModel>(context);
    final s = c.scale;

    mouseRegion({child}) => Obx(() => MouseRegion(
        cursor: cursorOverImage.isTrue
            ? c.cursorEmbedded
                ? SystemMouseCursors.none
                : keyboardEnabled.isTrue
                    ? (() {
                        if (remoteCursorMoved.isTrue) {
                          _lastRemoteCursorMoved = true;
                          return SystemMouseCursors.none;
                        } else {
                          if (_lastRemoteCursorMoved) {
                            _lastRemoteCursorMoved = false;
                            _firstEnterImage.value = true;
                          }
                          return _buildCustomCursor(context, s);
                        }
                      }())
                    : _buildDisabledCursor(context, s)
            : MouseCursor.defer,
        onHover: (evt) {},
        child: child));

    if (c.imageOverflow.isTrue && c.scrollStyle == ScrollStyle.scrollbar) {
      final imageWidth = c.getDisplayWidth() * s;
      final imageHeight = c.getDisplayHeight() * s;
      final imageSize = Size(imageWidth, imageHeight);
      final imageWidget = CustomPaint(
        size: imageSize,
        painter: ImagePainter(image: m.image, x: 0, y: 0, scale: s),
      );

      return NotificationListener<ScrollNotification>(
          onNotification: (notification) {
            final percentX = _horizontal.hasClients
                ? _horizontal.position.extentBefore /
                    (_horizontal.position.extentBefore +
                        _horizontal.position.extentInside +
                        _horizontal.position.extentAfter)
                : 0.0;
            final percentY = _vertical.hasClients
                ? _vertical.position.extentBefore /
                    (_vertical.position.extentBefore +
                        _vertical.position.extentInside +
                        _vertical.position.extentAfter)
                : 0.0;
            c.setScrollPercent(percentX, percentY);
            return false;
          },
          child: mouseRegion(
            child: Obx(() => _buildCrossScrollbarFromLayout(
                context, _buildListener(imageWidget), c.size, imageSize)),
          ));
    } else {
      final imageWidget = CustomPaint(
        size: Size(c.size.width, c.size.height),
        painter: ImagePainter(image: m.image, x: c.x / s, y: c.y / s, scale: s),
      );
      return mouseRegion(child: _buildListener(imageWidget));
    }
  }

  MouseCursor _buildCursorOfCache(
      CursorModel cursor, double scale, CursorData? cache) {
    if (cache == null) {
      return MouseCursor.defer;
    } else {
      final key = cache.updateGetKey(scale, zoomCursor.value);
      if (!cursor.cachedKeys.contains(key)) {
        debugPrint("Register custom cursor with key $key");
        // [Safety]
        // It's ok to call async registerCursor in current synchronous context,
        // because activating the cursor is also an async call and will always
        // be executed after this.
        custom_cursor_manager.CursorManager.instance
            .registerCursor(custom_cursor_manager.CursorData()
              ..buffer = cache.data!
              ..height = (cache.height * cache.scale).toInt()
              ..width = (cache.width * cache.scale).toInt()
              ..hotX = cache.hotx
              ..hotY = cache.hoty
              ..name = key);
        cursor.addKey(key);
      }
      return FlutterCustomMemoryImageCursor(key: key);
    }
  }

  MouseCursor _buildCustomCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = cursor.cache ?? preDefaultCursor.cache;
    return _buildCursorOfCache(cursor, scale, cache);
  }

  MouseCursor _buildDisabledCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = preForbiddenCursor.cache;
    return _buildCursorOfCache(cursor, scale, cache);
  }

  Widget _buildCrossScrollbarFromLayout(
      BuildContext context, Widget child, Size layoutSize, Size size) {
    final scrollConfig = CustomMouseWheelScrollConfig(
        scrollDuration: kDefaultScrollDuration,
        scrollCurve: Curves.linearToEaseOut,
        mouseWheelTurnsThrottleTimeMs:
            kDefaultMouseWheelThrottleDuration.inMilliseconds,
        scrollAmountMultiplier: kDefaultScrollAmountMultiplier);
    var widget = child;
    if (layoutSize.width < size.width) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: _horizontal,
          scrollDirection: Axis.horizontal,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Row(
        children: [
          Container(
            width: ((layoutSize.width - size.width) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.height < size.height) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: _vertical,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Column(
        children: [
          Container(
            height: ((layoutSize.height - size.height) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.width < size.width) {
      widget = ImprovedScrolling(
        scrollController: _horizontal,
        enableCustomMouseWheelScrolling: cursorOverImage.isFalse,
        customMouseWheelScrollConfig: scrollConfig,
        child: RawScrollbar(
          thumbColor: Colors.grey,
          controller: _horizontal,
          thumbVisibility: false,
          trackVisibility: false,
          notificationPredicate: layoutSize.height < size.height
              ? (notification) => notification.depth == 1
              : defaultScrollNotificationPredicate,
          child: widget,
        ),
      );
    }
    if (layoutSize.height < size.height) {
      widget = ImprovedScrolling(
        scrollController: _vertical,
        enableCustomMouseWheelScrolling: cursorOverImage.isFalse,
        customMouseWheelScrollConfig: scrollConfig,
        child: RawScrollbar(
          thumbColor: Colors.grey,
          controller: _vertical,
          thumbVisibility: false,
          trackVisibility: false,
          child: widget,
        ),
      );
    }

    return widget;
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
  final RxBool zoomCursor;

  const CursorPaint({
    Key? key,
    required this.id,
    required this.zoomCursor,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    double hotx = m.hotx;
    double hoty = m.hoty;
    if (m.image == null) {
      if (preDefaultCursor.image != null) {
        hotx = preDefaultCursor.image!.width / 2;
        hoty = preDefaultCursor.image!.height / 2;
      }
    }

    double cx = c.x;
    double cy = c.y;
    if (c.viewStyle.style == kRemoteViewStyleOriginal &&
        c.scrollStyle == ScrollStyle.scrollbar) {
      final d = c.parent.target!.ffiModel.display;
      final imageWidth = d.width * c.scale;
      final imageHeight = d.height * c.scale;
      cx = -imageWidth * c.scrollX;
      cy = -imageHeight * c.scrollY;
    }

    double x = (m.x - hotx) * c.scale + cx;
    double y = (m.y - hoty) * c.scale + cy;
    double scale = 1.0;
    if (zoomCursor.isTrue) {
      x = m.x - hotx + cx / c.scale;
      y = m.y - hoty + cy / c.scale;
      scale = c.scale;
    }

    return CustomPaint(
      painter: ImagePainter(
        image: m.image ?? preDefaultCursor.image,
        x: x,
        y: y,
        scale: scale,
      ),
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
    if ((scale - 1.0).abs() > 0.001) {
      paint.filterQuality = FilterQuality.medium;
      if (scale > 10.00000) {
        paint.filterQuality = FilterQuality.high;
      }
    }
    canvas.drawImage(
        image!, Offset(x.toInt().toDouble(), y.toInt().toDouble()), paint);
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}
