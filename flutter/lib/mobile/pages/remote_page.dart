import 'dart:async';
import 'dart:convert';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/mobile/widgets/gesture_help.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:get/get_state_manager/src/rx_flutter/rx_obx_widget.dart';
import 'package:provider/provider.dart';
import 'package:wakelock/wakelock.dart';

import '../../common.dart';
import '../../common/widgets/overlay.dart';
import '../../common/widgets/remote_input.dart';
import '../../models/input_model.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../widgets/dialog.dart';
import '../widgets/gestures.dart';

final initText = '\1' * 1024;

class RemotePage extends StatefulWidget {
  RemotePage({Key? key, required this.id}) : super(key: key);

  final String id;

  @override
  State<RemotePage> createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  Timer? _interval;
  Timer? _timer;
  bool _showBar = !isWebDesktop;
  double _bottom = 0;
  String _value = '';
  double _scale = 1;
  double _mouseScrollIntegral = 0; // mouse scroll speed controller
  Orientation? _currentOrientation;

  var _more = true;
  var _fn = false;
  final FocusNode _mobileFocusNode = FocusNode();
  final FocusNode _physicalFocusNode = FocusNode();
  var _showEdit = false; // use soft keyboard

  InputModel get inputModel => gFFI.inputModel;

  @override
  void initState() {
    super.initState();
    gFFI.start(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
    Wakelock.enable();
    _physicalFocusNode.requestFocus();
    gFFI.ffiModel.updateEventListener(widget.id);
    gFFI.inputModel.listenToMouse(true);
    gFFI.qualityMonitorModel.checkShowQualityMonitor(widget.id);
  }

  @override
  void dispose() {
    gFFI.dialogManager.hideMobileActionsOverlay();
    gFFI.inputModel.listenToMouse(false);
    gFFI.invokeMethod("enable_soft_keyboard", true);
    _mobileFocusNode.dispose();
    _physicalFocusNode.dispose();
    gFFI.close();
    _interval?.cancel();
    _timer?.cancel();
    gFFI.dialogManager.dismissAll();
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    Wakelock.disable();
    super.dispose();
  }

  void resetTool() {
    inputModel.resetModifiers();
  }

  bool isKeyboardShown() {
    return _bottom >= 100;
  }

  // crash on web before widget initiated.
  void intervalUnsafe() {
    var v = MediaQuery.of(context).viewInsets.bottom;
    if (v != _bottom) {
      resetTool();
      setState(() {
        _bottom = v;
        if (v < 100) {
          SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
              overlays: []);
          // [pi.version.isNotEmpty] -> check ready or not, avoid login without soft-keyboard
          if (gFFI.chatModel.chatWindowOverlayEntry == null &&
              gFFI.ffiModel.pi.version.isNotEmpty) {
            gFFI.invokeMethod("enable_soft_keyboard", false);
          }
        }
      });
    }
  }

  void interval() {
    try {
      intervalUnsafe();
    } catch (e) {}
  }

  // handle mobile virtual keyboard
  void handleSoftKeyboardInput(String newValue) {
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
        inputModel.inputKey('VK_BACK');
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
      final char = 'VK_BACK';
      inputModel.inputKey(char);
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
          openKeyboard();
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
    inputModel.inputKey(char);
  }

  void openKeyboard() {
    gFFI.invokeMethod("enable_soft_keyboard", true);
    // destroy first, so that our _value trick can work
    _value = initText;
    setState(() => _showEdit = false);
    _timer?.cancel();
    _timer = Timer(Duration(milliseconds: 30), () {
      // show now, and sleep a while to requestFocus to
      // make sure edit ready, so that keyboard wont show/hide/show/hide happen
      setState(() => _showEdit = true);
      _timer?.cancel();
      _timer = Timer(Duration(milliseconds: 30), () {
        SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
            overlays: SystemUiOverlay.values);
        _mobileFocusNode.requestFocus();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final pi = Provider.of<FfiModel>(context).pi;
    final hideKeyboard = isKeyboardShown() && _showEdit;
    final showActionButton = !_showBar || hideKeyboard;
    final keyboard = gFFI.ffiModel.permissions['keyboard'] != false;

    return WillPopScope(
      onWillPop: () async {
        clientClose(widget.id, gFFI.dialogManager);
        return false;
      },
      child: getRawPointerAndKeyBody(Scaffold(
          // resizeToAvoidBottomInset: true,
          floatingActionButton: !showActionButton
              ? null
              : FloatingActionButton(
                  mini: !hideKeyboard,
                  child: Icon(
                      hideKeyboard ? Icons.expand_more : Icons.expand_less),
                  backgroundColor: MyTheme.accent,
                  onPressed: () {
                    setState(() {
                      if (hideKeyboard) {
                        _showEdit = false;
                        gFFI.invokeMethod("enable_soft_keyboard", false);
                        _mobileFocusNode.unfocus();
                        _physicalFocusNode.requestFocus();
                      } else {
                        _showBar = !_showBar;
                      }
                    });
                  }),
          bottomNavigationBar: _showBar && pi.displays.isNotEmpty
              ? getBottomAppBar(keyboard)
              : null,
          body: Overlay(
            initialEntries: [
              OverlayEntry(builder: (context) {
                return Container(
                    color: Colors.black,
                    child: isWebDesktop
                        ? getBodyForDesktopWithListener(keyboard)
                        : SafeArea(child:
                            OrientationBuilder(builder: (ctx, orientation) {
                            if (_currentOrientation != orientation) {
                              Timer(const Duration(milliseconds: 200), () {
                                gFFI.dialogManager
                                    .resetMobileActionsOverlay(ffi: gFFI);
                                _currentOrientation = orientation;
                                gFFI.canvasModel.updateViewStyle();
                              });
                            }
                            return Obx(() => Container(
                                color: MyTheme.canvasColor,
                                child: inputModel.isPhysicalMouse.value
                                    ? getBodyForMobile()
                                    : getBodyForMobileWithGesture()));
                          })));
              })
            ],
          ))),
    );
  }

  Widget getRawPointerAndKeyBody(Widget child) {
    final keyboard = gFFI.ffiModel.permissions['keyboard'] != false;
    return RawPointerMouseRegion(
        cursor: keyboard ? SystemMouseCursors.none : MouseCursor.defer,
        inputModel: inputModel,
        child: RawKeyFocusScope(
            focusNode: _physicalFocusNode,
            inputModel: inputModel,
            child: child));
  }

  Widget getBottomAppBar(bool keyboard) {
    return BottomAppBar(
      elevation: 10,
      color: MyTheme.accent,
      child: Row(
        mainAxisSize: MainAxisSize.max,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: <Widget>[
          Row(
              children: <Widget>[
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.clear),
                      onPressed: () {
                        clientClose(widget.id, gFFI.dialogManager);
                      },
                    )
                  ] +
                  <Widget>[
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.tv),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showOptions(context, widget.id, gFFI.dialogManager);
                      },
                    )
                  ] +
                  (isWebDesktop
                      ? []
                      : gFFI.ffiModel.isPeerAndroid
                          ? [
                              IconButton(
                                color: Colors.white,
                                icon: const Icon(Icons.build),
                                onPressed: () => gFFI.dialogManager
                                    .toggleMobileActionsOverlay(ffi: gFFI),
                              )
                            ]
                          : [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: Icon(gFFI.ffiModel.touchMode
                                    ? Icons.touch_app
                                    : Icons.mouse),
                                onPressed: changeTouchMode,
                              ),
                            ]) +
                  (isWeb
                      ? []
                      : <Widget>[
                          IconButton(
                            color: Colors.white,
                            icon: Icon(Icons.message),
                            onPressed: () {
                              gFFI.chatModel
                                  .changeCurrentID(ChatModel.clientModeID);
                              gFFI.chatModel.toggleChatOverlay();
                            },
                          )
                        ]) +
                  [
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.more_vert),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showActions(widget.id);
                      },
                    ),
                  ]),
          IconButton(
              color: Colors.white,
              icon: Icon(Icons.expand_more),
              onPressed: () {
                setState(() => _showBar = !_showBar);
              }),
        ],
      ),
    );
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

  Offset _cacheLongPressPosition = Offset(0, 0);
  Widget getBodyForMobileWithGesture() {
    final touchMode = gFFI.ffiModel.touchMode;
    return getMixinGestureDetector(
        child: getBodyForMobile(),
        onTapUp: (d) {
          if (touchMode) {
            gFFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            inputModel.tap(MouseButtons.left);
          } else {
            inputModel.tap(MouseButtons.left);
          }
        },
        onDoubleTapDown: (d) {
          if (touchMode) {
            gFFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
          }
        },
        onDoubleTap: () {
          inputModel.tap(MouseButtons.left);
          inputModel.tap(MouseButtons.left);
        },
        onLongPressDown: (d) {
          if (touchMode) {
            gFFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            _cacheLongPressPosition = d.localPosition;
          }
        },
        onLongPress: () {
          if (touchMode) {
            gFFI.cursorModel
                .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
          }
          inputModel.tap(MouseButtons.right);
        },
        onDoubleFinerTap: (d) {
          if (!touchMode) {
            inputModel.tap(MouseButtons.right);
          }
        },
        onHoldDragStart: (d) {
          if (!touchMode) {
            inputModel.sendMouse('down', MouseButtons.left);
          }
        },
        onHoldDragUpdate: (d) {
          if (!touchMode) {
            gFFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, touchMode);
          }
        },
        onHoldDragEnd: (_) {
          if (!touchMode) {
            inputModel.sendMouse('up', MouseButtons.left);
          }
        },
        onOneFingerPanStart: (d) {
          if (touchMode) {
            gFFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            inputModel.sendMouse('down', MouseButtons.left);
          } else {
            final offset = gFFI.cursorModel.offset;
            final cursorX = offset.dx;
            final cursorY = offset.dy;
            final visible =
                gFFI.cursorModel.getVisibleRect().inflate(1); // extend edges
            final size = MediaQueryData.fromWindow(ui.window).size;
            if (!visible.contains(Offset(cursorX, cursorY))) {
              gFFI.cursorModel.move(size.width / 2, size.height / 2);
            }
          }
        },
        onOneFingerPanUpdate: (d) {
          gFFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, touchMode);
        },
        onOneFingerPanEnd: (d) {
          if (touchMode) {
            inputModel.sendMouse('up', MouseButtons.left);
          }
        },
        // scale + pan event
        onTwoFingerScaleUpdate: (d) {
          gFFI.canvasModel.updateScale(d.scale / _scale);
          _scale = d.scale;
          gFFI.canvasModel.panX(d.focalPointDelta.dx);
          gFFI.canvasModel.panY(d.focalPointDelta.dy);
        },
        onTwoFingerScaleEnd: (d) {
          _scale = 1;
          bind.sessionSetViewStyle(id: widget.id, value: "");
        },
        onThreeFingerVerticalDragUpdate: gFFI.ffiModel.isPeerAndroid
            ? null
            : (d) {
                _mouseScrollIntegral += d.delta.dy / 4;
                if (_mouseScrollIntegral > 1) {
                  inputModel.scroll(1);
                  _mouseScrollIntegral = 0;
                } else if (_mouseScrollIntegral < -1) {
                  inputModel.scroll(-1);
                  _mouseScrollIntegral = 0;
                }
              });
  }

  Widget getBodyForMobile() {
    return Container(
        color: MyTheme.canvasColor,
        child: Stack(children: () {
          final paints = [
            ImagePaint(),
            QualityMonitor(gFFI.qualityMonitorModel),
            getHelpTools(),
            SizedBox(
              width: 0,
              height: 0,
              child: !_showEdit
                  ? Container()
                  : TextFormField(
                      textInputAction: TextInputAction.newline,
                      autocorrect: false,
                      enableSuggestions: false,
                      autofocus: true,
                      focusNode: _mobileFocusNode,
                      maxLines: null,
                      initialValue: _value,
                      // trick way to make backspace work always
                      keyboardType: TextInputType.multiline,
                      onChanged: handleSoftKeyboardInput,
                    ),
            ),
          ];
          if (!gFFI.canvasModel.cursorEmbedded) {
            paints.add(CursorPaint());
          }
          return paints;
        }()));
  }

  Widget getBodyForDesktopWithListener(bool keyboard) {
    var paints = <Widget>[ImagePaint()];
    if (!gFFI.canvasModel.cursorEmbedded) {
      final cursor = bind.sessionGetToggleOptionSync(
          id: widget.id, arg: 'show-remote-cursor');
      if (keyboard || cursor) {
        paints.add(CursorPaint());
      }
    }
    return Container(
        color: MyTheme.canvasColor, child: Stack(children: paints));
  }

  void showActions(String id) async {
    final size = MediaQuery.of(context).size;
    final x = 120.0;
    final y = size.height;
    final more = <PopupMenuItem<String>>[];
    final pi = gFFI.ffiModel.pi;
    final perms = gFFI.ffiModel.permissions;
    if (pi.version.isNotEmpty) {
      more.add(PopupMenuItem<String>(
          child: Text(translate('Refresh')), value: 'refresh'));
    }
    more.add(PopupMenuItem<String>(
        child: Row(
            children: ([
          Text(translate('OS Password')),
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              showSetOSPassword(id, false, gFFI.dialogManager);
            },
            child: Icon(Icons.edit, color: MyTheme.accent),
          )
        ])),
        value: 'enter_os_password'));
    if (!isWebDesktop) {
      if (perms['keyboard'] != false && perms['clipboard'] != false) {
        more.add(PopupMenuItem<String>(
            child: Text(translate('Paste')), value: 'paste'));
      }
      more.add(PopupMenuItem<String>(
          child: Text(translate('Reset canvas')), value: 'reset_canvas'));
    }
    if (perms['keyboard'] != false) {
      more.add(PopupMenuItem<String>(
          child: Text(translate('Physical Keyboard Input Mode')),
          value: 'input-mode'));
      if (pi.platform == kPeerPlatformLinux || pi.sasEnabled) {
        more.add(PopupMenuItem<String>(
            child: Text('${translate('Insert')} Ctrl + Alt + Del'),
            value: 'cad'));
      }
      more.add(PopupMenuItem<String>(
          child: Text(translate('Insert Lock')), value: 'lock'));
      if (pi.platform == kPeerPlatformWindows &&
          await bind.sessionGetToggleOption(id: id, arg: 'privacy-mode') !=
              true) {
        more.add(PopupMenuItem<String>(
            child: Text(translate(
                '${gFFI.ffiModel.inputBlocked ? 'Unb' : 'B'}lock user input')),
            value: 'block-input'));
      }
    }
    if (perms["restart"] != false &&
        (pi.platform == kPeerPlatformLinux ||
            pi.platform == kPeerPlatformWindows ||
            pi.platform == kPeerPlatformMacOS)) {
      more.add(PopupMenuItem<String>(
          child: Text(translate('Restart Remote Device')), value: 'restart'));
    }
    // Currently only support VP9
    if (gFFI.recordingModel.start ||
        (perms["recording"] != false &&
            gFFI.qualityMonitorModel.data.codecFormat == "VP9")) {
      more.add(PopupMenuItem<String>(
          child: Row(
            children: [
              Text(translate(gFFI.recordingModel.start
                  ? 'Stop session recording'
                  : 'Start session recording')),
              Padding(
                padding: EdgeInsets.only(left: 12),
                child: Icon(
                    gFFI.recordingModel.start
                        ? Icons.pause_circle_filled
                        : Icons.videocam_outlined,
                    color: MyTheme.accent),
              )
            ],
          ),
          value: 'record'));
    }
    () async {
      var value = await showMenu(
        context: context,
        position: RelativeRect.fromLTRB(x, y, x, y),
        items: more,
        elevation: 8,
      );
      if (value == 'cad') {
        bind.sessionCtrlAltDel(id: widget.id);
      } else if (value == 'input-mode') {
        changePhysicalKeyboardInputMode();
      } else if (value == 'lock') {
        bind.sessionLockScreen(id: widget.id);
      } else if (value == 'block-input') {
        bind.sessionToggleOption(
            id: widget.id,
            value: '${gFFI.ffiModel.inputBlocked ? 'un' : ''}block-input');
        gFFI.ffiModel.inputBlocked = !gFFI.ffiModel.inputBlocked;
      } else if (value == 'refresh') {
        bind.sessionRefresh(id: widget.id);
      } else if (value == 'paste') {
        () async {
          ClipboardData? data = await Clipboard.getData(Clipboard.kTextPlain);
          if (data != null && data.text != null) {
            bind.sessionInputString(id: widget.id, value: data.text ?? "");
          }
        }();
      } else if (value == 'enter_os_password') {
        // FIXME:
        // null means no session of id
        // empty string means no password
        var password = await bind.sessionGetOption(id: id, arg: 'os-password');
        if (password != null) {
          bind.sessionInputOsPassword(id: widget.id, value: password);
        } else {
          showSetOSPassword(id, true, gFFI.dialogManager);
        }
      } else if (value == 'reset_canvas') {
        gFFI.cursorModel.reset();
      } else if (value == 'restart') {
        showRestartRemoteDevice(pi, widget.id, gFFI.dialogManager);
      } else if (value == 'record') {
        gFFI.recordingModel.toggle();
      }
    }();
  }

  void changeTouchMode() {
    setState(() => _showEdit = false);
    showModalBottomSheet(
        // backgroundColor: MyTheme.grayBg,
        isScrollControlled: true,
        context: context,
        shape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.vertical(top: Radius.circular(5))),
        builder: (context) => DraggableScrollableSheet(
            expand: false,
            builder: (context, scrollController) {
              return SingleChildScrollView(
                  controller: ScrollController(),
                  padding: EdgeInsets.symmetric(vertical: 10),
                  child: GestureHelp(
                      touchMode: gFFI.ffiModel.touchMode,
                      onTouchModeChange: (t) {
                        gFFI.ffiModel.toggleTouchMode();
                        final v = gFFI.ffiModel.touchMode ? 'Y' : '';
                        bind.sessionPeerOption(
                            id: widget.id, name: "touch", value: v);
                      }));
            }));
  }

  void changePhysicalKeyboardInputMode() async {
    var current = await bind.sessionGetKeyboardMode(id: widget.id) ?? "legacy";
    gFFI.dialogManager.show((setState, close) {
      void setMode(String? v) async {
        await bind.sessionPeerOption(
            id: widget.id, name: "keyboard-mode", value: v ?? "");
        setState(() => current = v ?? '');
        Future.delayed(Duration(milliseconds: 300), close);
      }

      return CustomAlertDialog(
          title: Text(translate('Physical Keyboard Input Mode')),
          content: Column(mainAxisSize: MainAxisSize.min, children: [
            getRadio('Legacy mode', 'legacy', current, setMode,
                contentPadding: EdgeInsets.zero),
            getRadio('Map mode', 'map', current, setMode,
                contentPadding: EdgeInsets.zero),
          ]));
    }, clickMaskDismiss: true);
  }

  Widget getHelpTools() {
    final keyboard = isKeyboardShown();
    if (!keyboard) {
      return SizedBox();
    }
    final size = MediaQuery.of(context).size;
    wrap(String text, void Function() onPressed,
        [bool? active, IconData? icon]) {
      return TextButton(
          style: TextButton.styleFrom(
            minimumSize: Size(0, 0),
            padding: EdgeInsets.symmetric(vertical: 10, horizontal: 9.75),
            //adds padding inside the button
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            //limits the touch area to the button area
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(5.0),
            ),
            backgroundColor: active == true ? MyTheme.accent80 : null,
          ),
          child: icon != null
              ? Icon(icon, size: 17, color: Colors.white)
              : Text(translate(text),
                  style: TextStyle(color: Colors.white, fontSize: 11)),
          onPressed: onPressed);
    }

    final pi = gFFI.ffiModel.pi;
    final isMac = pi.platform == kPeerPlatformMacOS;
    final modifiers = <Widget>[
      wrap('Ctrl ', () {
        setState(() => inputModel.ctrl = !inputModel.ctrl);
      }, inputModel.ctrl),
      wrap(' Alt ', () {
        setState(() => inputModel.alt = !inputModel.alt);
      }, inputModel.alt),
      wrap('Shift', () {
        setState(() => inputModel.shift = !inputModel.shift);
      }, inputModel.shift),
      wrap(isMac ? ' Cmd ' : ' Win ', () {
        setState(() => inputModel.command = !inputModel.command);
      }, inputModel.command),
    ];
    final keys = <Widget>[
      wrap(
          ' Fn ',
          () => setState(
                () {
                  _fn = !_fn;
                  if (_fn) {
                    _more = false;
                  }
                },
              ),
          _fn),
      wrap(
          ' ... ',
          () => setState(
                () {
                  _more = !_more;
                  if (_more) {
                    _fn = false;
                  }
                },
              ),
          _more),
    ];
    final fn = <Widget>[
      SizedBox(width: 9999),
    ];
    for (var i = 1; i <= 12; ++i) {
      final name = 'F$i';
      fn.add(wrap(name, () {
        inputModel.inputKey('VK_$name');
      }));
    }
    final more = <Widget>[
      SizedBox(width: 9999),
      wrap('Esc', () {
        inputModel.inputKey('VK_ESCAPE');
      }),
      wrap('Tab', () {
        inputModel.inputKey('VK_TAB');
      }),
      wrap('Home', () {
        inputModel.inputKey('VK_HOME');
      }),
      wrap('End', () {
        inputModel.inputKey('VK_END');
      }),
      wrap('Del', () {
        inputModel.inputKey('VK_DELETE');
      }),
      wrap('PgUp', () {
        inputModel.inputKey('VK_PRIOR');
      }),
      wrap('PgDn', () {
        inputModel.inputKey('VK_NEXT');
      }),
      SizedBox(width: 9999),
      wrap('', () {
        inputModel.inputKey('VK_LEFT');
      }, false, Icons.keyboard_arrow_left),
      wrap('', () {
        inputModel.inputKey('VK_UP');
      }, false, Icons.keyboard_arrow_up),
      wrap('', () {
        inputModel.inputKey('VK_DOWN');
      }, false, Icons.keyboard_arrow_down),
      wrap('', () {
        inputModel.inputKey('VK_RIGHT');
      }, false, Icons.keyboard_arrow_right),
      wrap(isMac ? 'Cmd+C' : 'Ctrl+C', () {
        sendPrompt(isMac, 'VK_C');
      }),
      wrap(isMac ? 'Cmd+V' : 'Ctrl+V', () {
        sendPrompt(isMac, 'VK_V');
      }),
      wrap(isMac ? 'Cmd+S' : 'Ctrl+S', () {
        sendPrompt(isMac, 'VK_S');
      }),
    ];
    final space = size.width > 320 ? 4.0 : 2.0;
    return Container(
        color: Color(0xAA000000),
        padding: EdgeInsets.only(
            top: keyboard ? 24 : 4, left: 0, right: 0, bottom: 8),
        child: Wrap(
          spacing: space,
          runSpacing: space,
          children: <Widget>[SizedBox(width: 9999)] +
              (keyboard
                  ? modifiers + keys + (_fn ? fn : []) + (_more ? more : [])
                  : modifiers),
        ));
  }
}

class ImagePaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    final c = Provider.of<CanvasModel>(context);
    final adjust = gFFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    return CustomPaint(
      painter: ImagePainter(
          image: m.image, x: c.x / s, y: (c.y - adjust) / s, scale: s),
    );
  }
}

class CursorPaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    final adjust = gFFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    double hotx = m.hotx;
    double hoty = m.hoty;
    if (m.image == null) {
      if (preDefaultCursor.image != null) {
        hotx = preDefaultCursor.image!.width / 2;
        hoty = preDefaultCursor.image!.height / 2;
      }
    }
    return CustomPaint(
      painter: ImagePainter(
          image: m.image ?? preDefaultCursor.image,
          x: m.x * s - hotx * s + c.x,
          y: m.y * s - hoty * s + c.y - adjust,
          scale: 1),
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
    canvas.scale(scale, scale);
    canvas.drawImage(image!, Offset(x, y), Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}

void showOptions(
    BuildContext context, String id, OverlayDialogManager dialogManager) async {
  String quality =
      await bind.sessionGetImageQuality(id: id) ?? kRemoteImageQualityBalanced;
  if (quality == '') quality = kRemoteImageQualityBalanced;
  String codec =
      await bind.sessionGetOption(id: id, arg: 'codec-preference') ?? 'auto';
  if (codec == '') codec = 'auto';
  String viewStyle = await bind.sessionGetViewStyle(id: id) ?? '';

  var displays = <Widget>[];
  final pi = gFFI.ffiModel.pi;
  final image = gFFI.ffiModel.getConnectionImage();
  if (image != null) {
    displays.add(Padding(padding: const EdgeInsets.only(top: 8), child: image));
  }
  if (pi.displays.length > 1) {
    final cur = pi.currentDisplay;
    final children = <Widget>[];
    for (var i = 0; i < pi.displays.length; ++i) {
      children.add(InkWell(
          onTap: () {
            if (i == cur) return;
            bind.sessionSwitchDisplay(id: id, value: i);
            gFFI.dialogManager.dismissAll();
          },
          child: Ink(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                  border: Border.all(color: Theme.of(context).hintColor),
                  borderRadius: BorderRadius.circular(2),
                  color: i == cur
                      ? Theme.of(context).toggleableActiveColor.withOpacity(0.6)
                      : null),
              child: Center(
                  child: Text((i + 1).toString(),
                      style: TextStyle(
                          color: i == cur ? Colors.white : Colors.black87,
                          fontWeight: FontWeight.bold))))));
    }
    displays.add(Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Wrap(
          alignment: WrapAlignment.center,
          spacing: 8,
          children: children,
        )));
  }
  if (displays.isNotEmpty) {
    displays.add(const Divider(color: MyTheme.border));
  }
  final perms = gFFI.ffiModel.permissions;
  final hasHwcodec = bind.mainHasHwcodec();
  final List<bool> codecs = [];
  if (hasHwcodec) {
    try {
      final Map codecsJson =
          jsonDecode(await bind.sessionSupportedHwcodec(id: id));
      final h264 = codecsJson['h264'] ?? false;
      final h265 = codecsJson['h265'] ?? false;
      codecs.add(h264);
      codecs.add(h265);
    } catch (e) {
      debugPrint("Show Codec Preference err=$e");
    }
  }

  dialogManager.show((setState, close) {
    final more = <Widget>[];
    if (perms['audio'] != false) {
      more.add(getToggle(id, setState, 'disable-audio', 'Mute'));
    }
    if (perms['keyboard'] != false) {
      if (perms['clipboard'] != false) {
        more.add(
            getToggle(id, setState, 'disable-clipboard', 'Disable clipboard'));
      }
      more.add(getToggle(
          id, setState, 'lock-after-session-end', 'Lock after session end'));
      if (pi.platform == kPeerPlatformWindows) {
        more.add(getToggle(id, setState, 'privacy-mode', 'Privacy mode'));
      }
    }
    setQuality(String? value) {
      if (value == null) return;
      setState(() {
        quality = value;
        bind.sessionSetImageQuality(id: id, value: value);
      });
    }

    setViewStyle(String? value) {
      if (value == null) return;
      setState(() {
        viewStyle = value;
        bind
            .sessionSetViewStyle(id: id, value: value)
            .then((_) => gFFI.canvasModel.updateViewStyle());
      });
    }

    setCodec(String? value) {
      if (value == null) return;
      setState(() {
        codec = value;
        bind
            .sessionPeerOption(id: id, name: "codec-preference", value: value)
            .then((_) => bind.sessionChangePreferCodec(id: id));
      });
    }

    final radios = [
      getRadio(
          'Scale original', kRemoteViewStyleOriginal, viewStyle, setViewStyle),
      getRadio(
          'Scale adaptive', kRemoteViewStyleAdaptive, viewStyle, setViewStyle),
      const Divider(color: MyTheme.border),
      getRadio(
          'Good image quality', kRemoteImageQualityBest, quality, setQuality),
      getRadio('Balanced', kRemoteImageQualityBalanced, quality, setQuality),
      getRadio('Optimize reaction time', kRemoteImageQualityLow, quality,
          setQuality),
      const Divider(color: MyTheme.border)
    ];

    if (hasHwcodec && codecs.length == 2 && (codecs[0] || codecs[1])) {
      radios.addAll([
        getRadio(translate('Auto'), 'auto', codec, setCodec),
        getRadio('VP9', 'vp9', codec, setCodec),
      ]);
      if (codecs[0]) {
        radios.add(getRadio('H264', 'h264', codec, setCodec));
      }
      if (codecs[1]) {
        radios.add(getRadio('H265', 'h265', codec, setCodec));
      }
      radios.add(const Divider(color: MyTheme.border));
    }

    final toggles = [
      getToggle(id, setState, 'show-quality-monitor', 'Show quality monitor'),
    ];
    if (!gFFI.canvasModel.cursorEmbedded) {
      toggles.insert(0,
          getToggle(id, setState, 'show-remote-cursor', 'Show remote cursor'));
    }

    return CustomAlertDialog(
      content: Column(
          mainAxisSize: MainAxisSize.min,
          children: displays + radios + toggles + more),
      contentPadding: 0,
    );
  }, clickMaskDismiss: true, backDismiss: true);
}

void showSetOSPassword(
    String id, bool login, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  var password = await bind.sessionGetOption(id: id, arg: "os-password") ?? "";
  var autoLogin = await bind.sessionGetOption(id: id, arg: "auto-login") != "";
  controller.text = password;
  dialogManager.show((setState, close) {
    return CustomAlertDialog(
        title: Text(translate('OS Password')),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          PasswordWidget(controller: controller),
          CheckboxListTile(
            contentPadding: const EdgeInsets.all(0),
            dense: true,
            controlAffinity: ListTileControlAffinity.leading,
            title: Text(
              translate('Auto Login'),
            ),
            value: autoLogin,
            onChanged: (v) {
              if (v == null) return;
              setState(() => autoLogin = v);
            },
          ),
        ]),
        actions: [
          dialogButton('Cancel', onPressed: close, isOutline: true),
          dialogButton(
            'OK',
            onPressed: () {
              var text = controller.text.trim();
              bind.sessionPeerOption(id: id, name: "os-password", value: text);
              bind.sessionPeerOption(
                  id: id, name: "auto-login", value: autoLogin ? 'Y' : '');
              if (text != "" && login) {
                bind.sessionInputOsPassword(id: id, value: text);
              }
              close();
            },
          ),
        ]);
  });
}

void sendPrompt(bool isMac, String key) {
  final old = isMac ? gFFI.inputModel.command : gFFI.inputModel.ctrl;
  if (isMac) {
    gFFI.inputModel.command = true;
  } else {
    gFFI.inputModel.ctrl = true;
  }
  gFFI.inputModel.inputKey(key);
  if (isMac) {
    gFFI.inputModel.command = old;
  } else {
    gFFI.inputModel.ctrl = old;
  }
}
