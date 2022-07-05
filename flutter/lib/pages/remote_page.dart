import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/widgets/gesture_help.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'dart:async';
import 'package:wakelock/wakelock.dart';
import '../common.dart';
import '../widgets/gestures.dart';
import '../models/model.dart';
import '../widgets/dialog.dart';
import '../widgets/overlay.dart';

final initText = '\1' * 1024;

class RemotePage extends StatefulWidget {
  RemotePage({Key? key, required this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  Timer? _interval;
  Timer? _timer;
  bool _showBar = !isDesktop;
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
  var _isPhysicalMouse = false;

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      showLoading(translate('Connecting...'));
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
    Wakelock.enable();
    _physicalFocusNode.requestFocus();
    FFI.ffiModel.updateEventListener(widget.id);
    FFI.listenToMouse(true);
  }

  @override
  void dispose() {
    hideMobileActionsOverlay();
    FFI.listenToMouse(false);
    FFI.invokeMethod("enable_soft_keyboard", true);
    _mobileFocusNode.dispose();
    _physicalFocusNode.dispose();
    FFI.close();
    _interval?.cancel();
    _timer?.cancel();
    SmartDialog.dismiss();
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    Wakelock.disable();
    super.dispose();
  }

  void resetTool() {
    FFI.resetModifiers();
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
          // [pi.version.isNotEmpty] -> check ready or not,avoid login without soft-keyboard
          if (chatWindowOverlayEntry == null &&
              FFI.ffiModel.pi.version.isNotEmpty) {
            FFI.invokeMethod("enable_soft_keyboard", false);
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
        FFI.inputKey('VK_BACK');
      }
      if (newValue.length > common) {
        var s = newValue.substring(common);
        if (s.length > 1) {
          FFI.setByName('input_string', s);
        } else {
          inputChar(s);
        }
      }
      return;
    }
    if (oldValue.length > 0 &&
        newValue.length > 0 &&
        oldValue[0] == '\1' &&
        newValue[0] != '\1') {
      // clipboard
      oldValue = '';
    }
    if (newValue.length == oldValue.length) {
      // ?
    } else if (newValue.length < oldValue.length) {
      final char = 'VK_BACK';
      FFI.inputKey(char);
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
          FFI.setByName('input_string', content);
          openKeyboard();
          return;
        }
        FFI.setByName('input_string', content);
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
    FFI.inputKey(char);
  }

  void openKeyboard() {
    FFI.invokeMethod("enable_soft_keyboard", true);
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

  void sendRawKey(RawKeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = _logicalKeyMap[e.logicalKey.keyId] ??
        _physicalKeyMap[e.physicalKey.usbHidUsage] ??
        e.logicalKey.keyLabel;
    FFI.inputKey(label, down: down, press: press ?? false);
  }

  @override
  Widget build(BuildContext context) {
    final pi = Provider.of<FfiModel>(context).pi;
    final hideKeyboard = isKeyboardShown() && _showEdit;
    final showActionButton = !_showBar || hideKeyboard;
    final keyboard = FFI.ffiModel.permissions['keyboard'] != false;

    return WillPopScope(
      onWillPop: () async {
        clientClose();
        return false;
      },
      child: getRawPointerAndKeyBody(
          keyboard,
          Scaffold(
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
                            FFI.invokeMethod("enable_soft_keyboard", false);
                            _mobileFocusNode.unfocus();
                            _physicalFocusNode.requestFocus();
                          } else {
                            _showBar = !_showBar;
                          }
                        });
                      }),
              bottomNavigationBar: _showBar && pi.displays.length > 0
                  ? getBottomAppBar(keyboard)
                  : null,
              body: Overlay(
                initialEntries: [
                  OverlayEntry(builder: (context) {
                    return Container(
                        color: Colors.black,
                        child: isDesktop
                            ? getBodyForDesktopWithListener(keyboard)
                            : SafeArea(child:
                                OrientationBuilder(builder: (ctx, orientation) {
                                if (_currentOrientation != orientation) {
                                  debugPrint("on orientation changed");
                                  Timer(Duration(milliseconds: 200), () {
                                    resetMobileActionsOverlay();
                                    _currentOrientation = orientation;
                                    FFI.canvasModel.updateViewStyle();
                                  });
                                }
                                return Container(
                                    color: MyTheme.canvasColor,
                                    child: _isPhysicalMouse
                                        ? getBodyForMobile()
                                        : getBodyForMobileWithGesture());
                              })));
                  })
                ],
              ))),
    );
  }

  Widget getRawPointerAndKeyBody(bool keyboard, Widget child) {
    return Listener(
        onPointerHover: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (!_isPhysicalMouse) {
            setState(() {
              _isPhysicalMouse = true;
            });
          }
          if (_isPhysicalMouse) {
            FFI.handleMouse(getEvent(e, 'mousemove'));
          }
        },
        onPointerDown: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) {
            if (_isPhysicalMouse) {
              setState(() {
                _isPhysicalMouse = false;
              });
            }
          }
          if (_isPhysicalMouse) {
            FFI.handleMouse(getEvent(e, 'mousedown'));
          }
        },
        onPointerUp: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (_isPhysicalMouse) {
            FFI.handleMouse(getEvent(e, 'mouseup'));
          }
        },
        onPointerMove: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (_isPhysicalMouse) {
            FFI.handleMouse(getEvent(e, 'mousemove'));
          }
        },
        onPointerSignal: (e) {
          if (e is PointerScrollEvent) {
            var dx = e.scrollDelta.dx;
            var dy = e.scrollDelta.dy;
            if (dx > 0)
              dx = -1;
            else if (dx < 0) dx = 1;
            if (dy > 0)
              dy = -1;
            else if (dy < 0) dy = 1;
            FFI.setByName(
                'send_mouse', '{"type": "wheel", "x": "$dx", "y": "$dy"}');
          }
        },
        child: MouseRegion(
            cursor: keyboard ? SystemMouseCursors.none : MouseCursor.defer,
            child: FocusScope(
                autofocus: true,
                child: Focus(
                    autofocus: true,
                    canRequestFocus: true,
                    focusNode: _physicalFocusNode,
                    onKey: (data, e) {
                      final key = e.logicalKey;
                      if (e is RawKeyDownEvent) {
                        if (e.repeat &&
                            !e.isAltPressed &&
                            !e.isControlPressed &&
                            !e.isShiftPressed &&
                            !e.isMetaPressed) {
                          sendRawKey(e, press: true);
                        } else {
                          sendRawKey(e, down: true);
                          if (e.isAltPressed && !FFI.alt) {
                            FFI.alt = true;
                          } else if (e.isControlPressed && !FFI.ctrl) {
                            FFI.ctrl = true;
                          } else if (e.isShiftPressed && !FFI.shift) {
                            FFI.shift = true;
                          } else if (e.isMetaPressed && !FFI.command) {
                            FFI.command = true;
                          }
                        }
                      }
                      // [!_showEdit] workaround for soft-keyboard's control_key like Backspace / Enter
                      if (!_showEdit && e is RawKeyUpEvent) {
                        if (key == LogicalKeyboardKey.altLeft ||
                            key == LogicalKeyboardKey.altRight) {
                          FFI.alt = false;
                        } else if (key == LogicalKeyboardKey.controlLeft ||
                            key == LogicalKeyboardKey.controlRight) {
                          FFI.ctrl = false;
                        } else if (key == LogicalKeyboardKey.shiftRight ||
                            key == LogicalKeyboardKey.shiftLeft) {
                          FFI.shift = false;
                        } else if (key == LogicalKeyboardKey.metaLeft ||
                            key == LogicalKeyboardKey.metaRight) {
                          FFI.command = false;
                        }
                        sendRawKey(e);
                      }
                      return KeyEventResult.handled;
                    },
                    child: child))));
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
                        clientClose();
                      },
                    )
                  ] +
                  <Widget>[
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.tv),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showOptions();
                      },
                    )
                  ] +
                  (isDesktop
                      ? []
                      : FFI.ffiModel.isPeerAndroid
                          ? [
                              IconButton(
                                color: Colors.white,
                                icon: Icon(Icons.build),
                                onPressed: () {
                                  if (mobileActionsOverlayEntry == null) {
                                    showMobileActionsOverlay();
                                  } else {
                                    hideMobileActionsOverlay();
                                  }
                                },
                              )
                            ]
                          : [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: Icon(FFI.ffiModel.touchMode
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
                              FFI.chatModel
                                  .changeCurrentID(ChatModel.clientModeID);
                              toggleChatOverlay();
                            },
                          )
                        ]) +
                  [
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.more_vert),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showActions();
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
    final touchMode = FFI.ffiModel.touchMode;
    return getMixinGestureDetector(
        child: getBodyForMobile(),
        onTapUp: (d) {
          if (touchMode) {
            FFI.cursorModel.touch(
                d.localPosition.dx, d.localPosition.dy, MouseButtons.left);
          } else {
            FFI.tap(MouseButtons.left);
          }
        },
        onDoubleTapDown: (d) {
          if (touchMode) {
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
          }
        },
        onDoubleTap: () {
          FFI.tap(MouseButtons.left);
          FFI.tap(MouseButtons.left);
        },
        onLongPressDown: (d) {
          if (touchMode) {
            _cacheLongPressPosition = d.localPosition;
          }
        },
        onLongPress: () {
          if (touchMode) {
            FFI.cursorModel
                .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
          }
          FFI.tap(MouseButtons.right);
        },
        onDoubleFinerTap: (d) {
          if (!touchMode) {
            FFI.tap(MouseButtons.right);
          }
        },
        onHoldDragStart: (d) {
          if (!touchMode) {
            FFI.sendMouse('down', MouseButtons.left);
          }
        },
        onHoldDragUpdate: (d) {
          if (!touchMode) {
            FFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, touchMode);
          }
        },
        onHoldDragEnd: (_) {
          if (!touchMode) {
            FFI.sendMouse('up', MouseButtons.left);
          }
        },
        onOneFingerPanStart: (d) {
          if (touchMode) {
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            FFI.sendMouse('down', MouseButtons.left);
          } else {
            final cursorX = FFI.cursorModel.x;
            final cursorY = FFI.cursorModel.y;
            final visible =
                FFI.cursorModel.getVisibleRect().inflate(1); // extend edges
            final size = MediaQueryData.fromWindow(ui.window).size;
            if (!visible.contains(Offset(cursorX, cursorY))) {
              FFI.cursorModel.move(size.width / 2, size.height / 2);
            }
          }
        },
        onOneFingerPanUpdate: (d) {
          FFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, touchMode);
        },
        onOneFingerPanEnd: (d) {
          if (touchMode) {
            FFI.sendMouse('up', MouseButtons.left);
          }
        },
        // scale + pan event
        onTwoFingerScaleUpdate: (d) {
          FFI.canvasModel.updateScale(d.scale / _scale);
          _scale = d.scale;
          FFI.canvasModel.panX(d.focalPointDelta.dx);
          FFI.canvasModel.panY(d.focalPointDelta.dy);
        },
        onTwoFingerScaleEnd: (d) {
          _scale = 1;
          FFI.setByName('peer_option', '{"name": "view-style", "value": ""}');
        },
        onThreeFingerVerticalDragUpdate: FFI.ffiModel.isPeerAndroid
            ? null
            : (d) {
                _mouseScrollIntegral += d.delta.dy / 4;
                if (_mouseScrollIntegral > 1) {
                  FFI.scroll(1);
                  _mouseScrollIntegral = 0;
                } else if (_mouseScrollIntegral < -1) {
                  FFI.scroll(-1);
                  _mouseScrollIntegral = 0;
                }
              });
  }

  Widget getBodyForMobile() {
    return Container(
        color: MyTheme.canvasColor,
        child: Stack(children: [
          ImagePaint(),
          CursorPaint(),
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
                    onChanged: handleInput,
                  ),
          ),
        ]));
  }

  Widget getBodyForDesktopWithListener(bool keyboard) {
    var paints = <Widget>[ImagePaint()];
    if (keyboard ||
        FFI.getByName('toggle_option', 'show-remote-cursor') == 'true') {
      paints.add(CursorPaint());
    }
    return Container(
        color: MyTheme.canvasColor, child: Stack(children: paints));
  }

  int lastMouseDownButtons = 0;
  Map<String, dynamic> getEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};
    out['type'] = type;
    out['x'] = evt.position.dx;
    out['y'] = evt.position.dy;
    if (FFI.alt) out['alt'] = 'true';
    if (FFI.shift) out['shift'] = 'true';
    if (FFI.ctrl) out['ctrl'] = 'true';
    if (FFI.command) out['command'] = 'true';
    out['buttons'] = evt
        .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
    if (evt.buttons != 0) {
      lastMouseDownButtons = evt.buttons;
    } else {
      out['buttons'] = lastMouseDownButtons;
    }
    return out;
  }

  void showActions() {
    final size = MediaQuery.of(context).size;
    final x = 120.0;
    final y = size.height;
    final more = <PopupMenuItem<String>>[];
    final pi = FFI.ffiModel.pi;
    final perms = FFI.ffiModel.permissions;
    if (pi.version.isNotEmpty) {
      more.add(PopupMenuItem<String>(
          child: Text(translate('Refresh')), value: 'refresh'));
    }
    more.add(PopupMenuItem<String>(
        child: Row(
            children: ([
          Container(width: 100.0, child: Text(translate('OS Password'))),
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              Navigator.pop(context);
              showSetOSPassword(false);
            },
            child: Icon(Icons.edit, color: MyTheme.accent),
          )
        ])),
        value: 'enter_os_password'));
    if (!isDesktop) {
      if (perms['keyboard'] != false && perms['clipboard'] != false) {
        more.add(PopupMenuItem<String>(
            child: Text(translate('Paste')), value: 'paste'));
      }
      more.add(PopupMenuItem<String>(
          child: Text(translate('Reset canvas')), value: 'reset_canvas'));
    }
    if (perms['keyboard'] != false) {
      if (pi.platform == 'Linux' || pi.sasEnabled) {
        more.add(PopupMenuItem<String>(
            child: Text(translate('Insert') + ' Ctrl + Alt + Del'),
            value: 'cad'));
      }
      more.add(PopupMenuItem<String>(
          child: Text(translate('Insert Lock')), value: 'lock'));
      if (pi.platform == 'Windows' &&
          FFI.getByName('toggle_option', 'privacy-mode') != 'true') {
        more.add(PopupMenuItem<String>(
            child: Text(translate(
                (FFI.ffiModel.inputBlocked ? 'Unb' : 'B') + 'lock user input')),
            value: 'block-input'));
      }
    }
    () async {
      var value = await showMenu(
        context: context,
        position: RelativeRect.fromLTRB(x, y, x, y),
        items: more,
        elevation: 8,
      );
      if (value == 'cad') {
        FFI.setByName('ctrl_alt_del');
      } else if (value == 'lock') {
        FFI.setByName('lock_screen');
      } else if (value == 'block-input') {
        FFI.setByName('toggle_option',
            (FFI.ffiModel.inputBlocked ? 'un' : '') + 'block-input');
        FFI.ffiModel.inputBlocked = !FFI.ffiModel.inputBlocked;
      } else if (value == 'refresh') {
        FFI.setByName('refresh');
      } else if (value == 'paste') {
        () async {
          ClipboardData? data = await Clipboard.getData(Clipboard.kTextPlain);
          if (data != null && data.text != null) {
            FFI.setByName('input_string', '${data.text}');
          }
        }();
      } else if (value == 'enter_os_password') {
        var password = FFI.getByName('peer_option', "os-password");
        if (password != "") {
          FFI.setByName('input_os_password', password);
        } else {
          showSetOSPassword(true);
        }
      } else if (value == 'reset_canvas') {
        FFI.cursorModel.reset();
      }
    }();
  }

  void changeTouchMode() {
    setState(() => _showEdit = false);
    showModalBottomSheet(
        backgroundColor: MyTheme.grayBg,
        isScrollControlled: true,
        context: context,
        shape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.vertical(top: Radius.circular(5))),
        builder: (context) => DraggableScrollableSheet(
            expand: false,
            builder: (context, scrollController) {
              return SingleChildScrollView(
                  padding: EdgeInsets.symmetric(vertical: 10),
                  child: GestureHelp(
                      touchMode: FFI.ffiModel.touchMode,
                      onTouchModeChange: (t) {
                        FFI.ffiModel.toggleTouchMode();
                        final v = FFI.ffiModel.touchMode ? 'Y' : '';
                        FFI.setByName('peer_option',
                            '{"name": "touch-mode", "value": "$v"}');
                      }));
            }));
  }

  Widget getHelpTools() {
    final keyboard = isKeyboardShown();
    if (!keyboard) {
      return SizedBox();
    }
    final size = MediaQuery.of(context).size;
    var wrap = (String text, void Function() onPressed,
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
    };
    final pi = FFI.ffiModel.pi;
    final isMac = pi.platform == "Mac OS";
    final modifiers = <Widget>[
      wrap('Ctrl ', () {
        setState(() => FFI.ctrl = !FFI.ctrl);
      }, FFI.ctrl),
      wrap(' Alt ', () {
        setState(() => FFI.alt = !FFI.alt);
      }, FFI.alt),
      wrap('Shift', () {
        setState(() => FFI.shift = !FFI.shift);
      }, FFI.shift),
      wrap(isMac ? ' Cmd ' : ' Win ', () {
        setState(() => FFI.command = !FFI.command);
      }, FFI.command),
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
      final name = 'F' + i.toString();
      fn.add(wrap(name, () {
        FFI.inputKey('VK_' + name);
      }));
    }
    final more = <Widget>[
      SizedBox(width: 9999),
      wrap('Esc', () {
        FFI.inputKey('VK_ESCAPE');
      }),
      wrap('Tab', () {
        FFI.inputKey('VK_TAB');
      }),
      wrap('Home', () {
        FFI.inputKey('VK_HOME');
      }),
      wrap('End', () {
        FFI.inputKey('VK_END');
      }),
      wrap('Del', () {
        FFI.inputKey('VK_DELETE');
      }),
      wrap('PgUp', () {
        FFI.inputKey('VK_PRIOR');
      }),
      wrap('PgDn', () {
        FFI.inputKey('VK_NEXT');
      }),
      SizedBox(width: 9999),
      wrap('', () {
        FFI.inputKey('VK_LEFT');
      }, false, Icons.keyboard_arrow_left),
      wrap('', () {
        FFI.inputKey('VK_UP');
      }, false, Icons.keyboard_arrow_up),
      wrap('', () {
        FFI.inputKey('VK_DOWN');
      }, false, Icons.keyboard_arrow_down),
      wrap('', () {
        FFI.inputKey('VK_RIGHT');
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
    final adjust = FFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    return CustomPaint(
      painter: new ImagePainter(
          image: m.image, x: c.x / s, y: (c.y - adjust) / s, scale: s),
    );
  }
}

class CursorPaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    final adjust = FFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    return CustomPaint(
      painter: new ImagePainter(
          image: m.image,
          x: m.x * s - m.hotx + c.x,
          y: m.y * s - m.hoty + c.y - adjust,
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
    canvas.drawImage(image!, new Offset(x, y), new Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}

CheckboxListTile getToggle(
    void Function(void Function()) setState, option, name) {
  return CheckboxListTile(
      value: FFI.getByName('toggle_option', option) == 'true',
      onChanged: (v) {
        setState(() {
          FFI.setByName('toggle_option', option);
        });
      },
      dense: true,
      title: Text(translate(name)));
}

RadioListTile<String> getRadio(String name, String toValue, String curValue,
    void Function(String?) onChange) {
  return RadioListTile<String>(
    controlAffinity: ListTileControlAffinity.trailing,
    title: Text(translate(name)),
    value: toValue,
    groupValue: curValue,
    onChanged: onChange,
    dense: true,
  );
}

void showOptions() {
  String quality = FFI.getByName('image_quality');
  if (quality == '') quality = 'balanced';
  String viewStyle = FFI.getByName('peer_option', 'view-style');
  var displays = <Widget>[];
  final pi = FFI.ffiModel.pi;
  final image = FFI.ffiModel.getConnectionImage();
  if (image != null)
    displays.add(Padding(padding: const EdgeInsets.only(top: 8), child: image));
  if (pi.displays.length > 1) {
    final cur = pi.currentDisplay;
    final children = <Widget>[];
    for (var i = 0; i < pi.displays.length; ++i)
      children.add(InkWell(
          onTap: () {
            if (i == cur) return;
            FFI.setByName('switch_display', i.toString());
            SmartDialog.dismiss();
          },
          child: Ink(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                  border: Border.all(color: Colors.black87),
                  color: i == cur ? Colors.black87 : Colors.white),
              child: Center(
                  child: Text((i + 1).toString(),
                      style: TextStyle(
                          color: i == cur ? Colors.white : Colors.black87))))));
    displays.add(Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Wrap(
          alignment: WrapAlignment.center,
          spacing: 8,
          children: children,
        )));
  }
  if (displays.isNotEmpty) {
    displays.add(Divider(color: MyTheme.border));
  }
  final perms = FFI.ffiModel.permissions;

  DialogManager.show((setState, close) {
    final more = <Widget>[];
    if (perms['audio'] != false) {
      more.add(getToggle(setState, 'disable-audio', 'Mute'));
    }
    if (perms['keyboard'] != false) {
      if (perms['clipboard'] != false)
        more.add(getToggle(setState, 'disable-clipboard', 'Disable clipboard'));
      more.add(getToggle(
          setState, 'lock-after-session-end', 'Lock after session end'));
      if (pi.platform == 'Windows') {
        more.add(getToggle(setState, 'privacy-mode', 'Privacy mode'));
      }
    }
    var setQuality = (String? value) {
      if (value == null) return;
      setState(() {
        quality = value;
        FFI.setByName('image_quality', value);
      });
    };
    var setViewStyle = (String? value) {
      if (value == null) return;
      setState(() {
        viewStyle = value;
        FFI.setByName(
            'peer_option', '{"name": "view-style", "value": "$value"}');
        FFI.canvasModel.updateViewStyle();
      });
    };
    return CustomAlertDialog(
      title: SizedBox.shrink(),
      content: Column(
          mainAxisSize: MainAxisSize.min,
          children: displays +
              <Widget>[
                getRadio('Original', 'original', viewStyle, setViewStyle),
                getRadio('Shrink', 'shrink', viewStyle, setViewStyle),
                getRadio('Stretch', 'stretch', viewStyle, setViewStyle),
                Divider(color: MyTheme.border),
                getRadio('Good image quality', 'best', quality, setQuality),
                getRadio('Balanced', 'balanced', quality, setQuality),
                getRadio('Optimize reaction time', 'low', quality, setQuality),
                Divider(color: MyTheme.border),
                getToggle(setState, 'show-remote-cursor', 'Show remote cursor'),
              ] +
              more),
      actions: [],
      contentPadding: 0,
    );
  }, clickMaskDismiss: true, backDismiss: true);
}

void showSetOSPassword(bool login) {
  final controller = TextEditingController();
  var password = FFI.getByName('peer_option', "os-password");
  var autoLogin = FFI.getByName('peer_option', "auto-login") != "";
  controller.text = password;
  DialogManager.show((setState, close) {
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
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              close();
            },
            child: Text(translate('Cancel')),
          ),
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              var text = controller.text.trim();
              FFI.setByName(
                  'peer_option', '{"name": "os-password", "value": "$text"}');
              FFI.setByName('peer_option',
                  '{"name": "auto-login", "value": "${autoLogin ? 'Y' : ''}"}');
              if (text != "" && login) {
                FFI.setByName('input_os_password', text);
              }
              close();
            },
            child: Text(translate('OK')),
          ),
        ]);
  });
}

void sendPrompt(bool isMac, String key) {
  final old = isMac ? FFI.command : FFI.ctrl;
  if (isMac) {
    FFI.command = true;
  } else {
    FFI.ctrl = true;
  }
  FFI.inputKey(key);
  if (isMac) {
    FFI.command = old;
  } else {
    FFI.ctrl = old;
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
