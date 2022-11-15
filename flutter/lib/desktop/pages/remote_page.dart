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
    with AutomaticKeepAliveClientMixin {
  Timer? _timer;
  String keyboardMode = "legacy";
  final _cursorOverImage = false.obs;
  late RxBool _showRemoteCursor;
  late RxBool _remoteCursorMoved;
  late RxBool _keyboardEnabled;

  final FocusNode _rawKeyFocusNode = FocusNode();
  var _imageFocused = false;

  Function(bool)? _onEnterOrLeaveImage4Menubar;

  late FFI _ffi;

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

    Get.put(_ffi, tag: widget.id);
    _ffi.start(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    if (!Platform.isLinux) {
      Wakelock.enable();
    }
    _rawKeyFocusNode.requestFocus();
    _ffi.ffiModel.updateEventListener(widget.id);
    _ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
    _showRemoteCursor.value = bind.sessionGetToggleOptionSync(
        id: widget.id, arg: 'show-remote-cursor');
    if (!_isCustomCursorInited) {
      customCursorController.registerNeedUpdateCursorCallback(
          (String? lastKey, String? currentKey) async {
        if (_firstEnterImage.value) {
          _firstEnterImage.value = false;
          return true;
        }
        return lastKey == null || lastKey != currentKey;
      });
      _isCustomCursorInited = true;
    }
  }

  @override
  void dispose() {
    debugPrint("REMOTE PAGE dispose ${widget.id}");
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
                      onFocusChange: (bool v) {
                        _imageFocused = v;
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
    if (!_imageFocused) {
      _rawKeyFocusNode.requestFocus();
    }
    _cursorOverImage.value = true;
    _firstEnterImage.value = true;
    if (_onEnterOrLeaveImage4Menubar != null) {
      try {
        _onEnterOrLeaveImage4Menubar!(true);
      } catch (e) {
        //
      }
    }
    _ffi.inputModel.enterOrLeave(true);
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
    _ffi.inputModel.enterOrLeave(false);
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
          listenerBuilder: (child) => RawPointerMouseRegion(
            onEnter: enterView,
            onExit: leaveView,
            inputModel: _ffi.inputModel,
            child: child,
          ),
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
  final Rx<bool> cursorOverImage;
  final Rx<bool> keyboardEnabled;
  final Rx<bool> remoteCursorMoved;
  final Widget Function(Widget)? listenerBuilder;

  ImagePaint(
      {Key? key,
      required this.id,
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
            ? keyboardEnabled.isTrue
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

    if (c.scrollStyle == ScrollStyle.scrollbar) {
      final imageWidth = c.getDisplayWidth() * s;
      final imageHeight = c.getDisplayHeight() * s;
      final imageWidget = SizedBox(
          width: imageWidth,
          height: imageHeight,
          child: CustomPaint(
            painter: ImagePainter(image: m.image, x: 0, y: 0, scale: s),
          ));

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
            child: _buildCrossScrollbar(context, _buildListener(imageWidget),
                Size(imageWidth, imageHeight))),
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

  MouseCursor _buildCustomCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = cursor.cache ?? cursor.defaultCache;
    if (cache == null) {
      return MouseCursor.defer;
    } else {
      final key = cache.updateGetKey(scale);
      cursor.addKey(key);
      return FlutterCustomMemoryImageCursor(
        pixbuf: cache.data,
        key: key,
        hotx: cache.hotx,
        hoty: cache.hoty,
        imageWidth: (cache.width * cache.scale).toInt(),
        imageHeight: (cache.height * cache.scale).toInt(),
      );
    }
  }

  MouseCursor _buildDisabledCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = cursor.cache;
    if (cache == null) {
      return MouseCursor.defer;
    } else {
      if (cursor.cachedForbidmemoryCursorData == null) {
        cursor.updateForbiddenCursorBuffer();
      }
      final key = 'disabled_cursor_key';
      cursor.addKey(key);
      return FlutterCustomMemoryImageCursor(
        pixbuf: cursor.cachedForbidmemoryCursorData,
        key: key,
        hotx: 0,
        hoty: 0,
        imageWidth: 32,
        imageHeight: 32,
      );
    }
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
        mainAxisAlignment: MainAxisAlignment.center,
        children: [widget],
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
        mainAxisAlignment: MainAxisAlignment.center,
        children: [widget],
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

  Widget _buildCrossScrollbar(BuildContext context, Widget child, Size size) {
    var layoutSize = MediaQuery.of(context).size;
    // If minimized, w or h may be negative here.
    final w = layoutSize.width - kWindowBorderWidth * 2;
    final h =
        layoutSize.height - kWindowBorderWidth * 2 - kDesktopRemoteTabBarHeight;
    layoutSize = Size(
      w < 0 ? 0 : w,
      h < 0 ? 0 : h,
    );
    bool overflow =
        layoutSize.width < size.width || layoutSize.height < size.height;
    return overflow
        ? Obx(() =>
            _buildCrossScrollbarFromLayout(context, child, layoutSize, size))
        : _buildCrossScrollbarFromLayout(context, child, layoutSize, size);
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
    double hotx = m.hotx;
    double hoty = m.hoty;
    if (m.image == null) {
      if (m.defaultCache != null) {
        hotx = m.defaultImage!.width / 2;
        hoty = m.defaultImage!.height / 2;
      }
    }
    return CustomPaint(
      painter: ImagePainter(
          image: m.image ?? m.defaultImage,
          x: m.x - hotx + c.x / c.scale,
          y: m.y - hoty + c.y / c.scale,
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
