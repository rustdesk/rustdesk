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
import 'chat_page.dart';

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

  var _more = true;
  var _fn = false;
  final FocusNode _mobileFocusNode = FocusNode();
  final FocusNode _physicalFocusNode = FocusNode();
  var _showEdit = false;
  var _touchMode = false;
  var _isPhysicalKeyboard = false;

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance!.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      showLoading(translate('Connecting...'));
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
    Wakelock.enable();
    _touchMode = FFI.getByName('peer_option', "touch-mode") != '';
    _physicalFocusNode.requestFocus();
    FFI.listenToMouse(true);
  }

  @override
  void dispose() {
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
          FFI.invokeMethod("enable_soft_keyboard", false);
        }
      });
    }
    FFI.ffiModel.update(widget.id);
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
          ++common);
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

  void sendRawKey(RawKeyEvent e, [bool? down]) {
    final label = _keyLabels[e.logicalKey.keyId];
    if (label != null) {
      FFI.inputKey(label, down);
    }
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
              bottomNavigationBar:
                  _showBar && pi.displays != null ? getBottomAppBar() : null,
              body: Overlay(
                initialEntries: [
                  OverlayEntry(builder: (context) {
                    return Container(
                        color: Colors.black,
                        // child: getRawPointerAndKeyBody(keyboard));
                        child: isDesktop
                            ? getBodyForDesktopWithListener(keyboard)
                            : SafeArea(
                                child: Container(
                                    color: MyTheme.canvasColor,
                                    child: _isPhysicalKeyboard
                                        ? getBodyForMobile()
                                        : getBodyForMobileWithGesture())));
                  })
                ],
              ))),
    );
  }

  Widget getRawPointerAndKeyBody(bool keyboard, Widget child) {
    return Listener(
        onPointerHover: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (!_isPhysicalKeyboard) {
            setState(() {
              _isPhysicalKeyboard = true;
            });
          }
          if (_isPhysicalKeyboard) {
            FFI.handleMouse(getEvent(e, 'mousemove'));
          }
        },
        onPointerDown: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) {
            if (_isPhysicalKeyboard) {
              setState(() {
                _isPhysicalKeyboard = false;
              });
            }
          }
          if (_isPhysicalKeyboard) {
            FFI.handleMouse(getEvent(e, 'mousedown'));
          }
        },
        onPointerUp: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (_isPhysicalKeyboard) {
            FFI.handleMouse(getEvent(e, 'mouseup'));
          }
        },
        onPointerMove: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) return;
          if (_isPhysicalKeyboard) {
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
                        if (e.isAltPressed && !FFI.alt) {
                          FFI.alt = true;
                          sendRawKey(e, true);
                        } else if (e.isControlPressed && !FFI.ctrl) {
                          FFI.ctrl = true;
                          sendRawKey(e, true);
                        } else if (e.isShiftPressed && !FFI.shift) {
                          FFI.shift = true;
                          sendRawKey(e, true);
                        } else if (e.isMetaPressed && !FFI.command) {
                          FFI.command = true;
                          sendRawKey(e, true);
                        } else if (e.repeat) {
                          sendRawKey(e);
                        }
                      }
                      if (e is RawKeyUpEvent) {
                        if (key == LogicalKeyboardKey.altLeft ||
                            key == LogicalKeyboardKey.altRight) {
                          FFI.alt = false;
                        }
                        if (key == LogicalKeyboardKey.controlLeft ||
                            key == LogicalKeyboardKey.controlRight) {
                          FFI.ctrl = false;
                        }
                        if (key == LogicalKeyboardKey.shiftRight ||
                            key == LogicalKeyboardKey.shiftLeft) {
                          FFI.shift = false;
                        }
                        if (key == LogicalKeyboardKey.metaLeft ||
                            key == LogicalKeyboardKey.metaRight) {
                          FFI.command = false;
                        }
                        sendRawKey(e);
                      }
                      return KeyEventResult.handled;
                    },
                    child: child))));
  }

  Widget getBottomAppBar() {
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
                      : [
                          IconButton(
                              color: Colors.white,
                              icon: Icon(Icons.keyboard),
                              onPressed: openKeyboard),
                          IconButton(
                            color: Colors.white,
                            icon: Icon(
                                _touchMode ? Icons.touch_app : Icons.mouse),
                            onPressed: changeTouchMode,
                          )
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

  Widget getBodyForMobileWithGesture() {
    return getMixinGestureDetector(
        child: getBodyForMobile(),
        onTapUp: (d) {
          if (_touchMode) {
            FFI.cursorModel.touch(
                d.localPosition.dx, d.localPosition.dy, MouseButtons.left);
          } else {
            FFI.tap(MouseButtons.left);
          }
        },
        onDoubleTapDown: (d) {
          if (_touchMode) {
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
          }
        },
        onDoubleTap: () {
          FFI.tap(MouseButtons.left);
          FFI.tap(MouseButtons.left);
        },
        onLongPressDown: (d) {
          if (_touchMode) {
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
          }
        },
        onLongPress: () {
          FFI.tap(MouseButtons.right);
        },
        onDoubleFinerTap: (d) {
          if (!_touchMode) {
            FFI.tap(MouseButtons.right);
          }
        },
        onHoldDragStart: (d) {
          if (!_touchMode) {
            FFI.sendMouse('down', MouseButtons.left);
          }
        },
        onHoldDragUpdate: (d) {
          if (!_touchMode) {
            FFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, _touchMode);
          }
        },
        onHoldDragEnd: (_) {
          if (!_touchMode) {
            FFI.sendMouse('up', MouseButtons.left);
          }
        },
        onOneFingerPanStart: (d) {
          if (_touchMode) {
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            FFI.sendMouse('down', MouseButtons.left);
          }
        },
        onOneFingerPanUpdate: (d) {
          FFI.cursorModel.updatePan(d.delta.dx, d.delta.dy, _touchMode);
        },
        onOneFingerPanEnd: (d) {
          if (_touchMode) {
            FFI.sendMouse('up', MouseButtons.left);
          }
        },
        onTwoFingerScaleUpdate: (d) {
          FFI.canvasModel.updateScale(d.scale / _scale);
          _scale = d.scale;
          FFI.canvasModel.panX(d.focalPointDelta.dx);
          FFI.canvasModel.panY(d.focalPointDelta.dy);
        },
        onTwoFingerScaleEnd: (d) => _scale = 1,
        onTwoFingerVerticalDragUpdate: (d) {
          FFI.scroll(d.delta.dy / 2);
        },
        onTwoFingerPanUpdate: (d) {
          FFI.canvasModel.panX(d.delta.dx);
          FFI.canvasModel.panY(d.delta.dy);
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
                      touchMode: _touchMode,
                      onTouchModeChange: (t) {
                        setState(() => _touchMode = t);
                        final v = _touchMode ? 'Y' : '';
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
  if (viewStyle == '') viewStyle = 'original';
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
const Map<int, String> _keyLabels = <int, String>{
  0x00000000020: 'VK_SPACE',
  0x00000000021: 'Exclamation',
  0x00000000022: 'VK_QUOTE',
  0x00000000023: 'Number Sign',
  0x00000000024: 'Dollar',
  0x00000000025: 'Percent',
  0x00000000026: 'Ampersand',
  0x00000000027: 'Quote Single',
  0x00000000028: 'Parenthesis Left',
  0x00000000029: 'Parenthesis Right',
  0x0000000002a: 'Asterisk',
  0x0000000002b: 'Add',
  0x0000000002c: 'VK_COMMA',
  0x0000000002d: 'VK_MINUS',
  0x0000000002e: 'Period',
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
  0x0000000003a: 'Colon',
  0x0000000003b: 'VK_SEMICOLON',
  0x0000000003c: 'Less',
  0x0000000003d: 'VK_PLUS', // it is =
  0x0000000003e: 'Greater',
  0x0000000003f: 'Question',
  0x00000000040: 'At',
  0x0000000005b: 'VK_LBRACKET',
  0x0000000005c: 'VK_BACKSLASH',
  0x0000000005d: 'VK_RBRACKET',
  0x0000000005e: 'Caret',
  0x0000000005f: 'Underscore',
  0x00000000060: 'Backquote',
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
  0x0000000007b: 'Brace Left',
  0x0000000007c: 'Bar',
  0x0000000007d: 'Brace Right',
  0x0000000007e: 'Tilde',
  0x00100000001: 'Unidentified',
  0x00100000008: 'VK_BACK',
  0x00100000009: 'VK_TAB',
  0x0010000000d: 'VK_ENTER',
  0x0010000001b: 'VK_ESCAPE',
  0x0010000007f: 'VK_DELETE',
  0x00100000101: 'Accel',
  0x00100000103: 'Alt Graph',
  0x00100000104: 'VK_CAPITAL',
  0x00100000106: 'Fn',
  0x00100000107: 'Fn Lock',
  0x00100000108: 'Hyper',
  0x0010000010a: 'Num Lock',
  0x0010000010c: 'Scroll Lock',
  0x0010000010e: 'Super',
  0x0010000010f: 'Symbol',
  0x00100000110: 'Symbol Lock',
  0x00100000111: 'Shift Level 5',
  0x00100000301: 'VK_DOWN',
  0x00100000302: 'VK_LEFT',
  0x00100000303: 'VK_RIGHT',
  0x00100000304: 'VK_UP',
  0x00100000305: 'VK_END',
  0x00100000306: 'VK_HOME',
  0x00100000307: 'VK_NEXT',
  0x00100000308: 'VK_PRIOR',
  0x00100000401: 'VK_CLEAR',
  0x00100000402: 'Copy',
  0x00100000403: 'Cr Sel',
  0x00100000404: 'Cut',
  0x00100000405: 'Erase Eof',
  0x00100000406: 'Ex Sel',
  0x00100000407: 'VK_INSERT',
  0x00100000408: 'Paste',
  0x00100000409: 'Redo',
  0x0010000040a: 'Undo',
  0x00100000501: 'Accept',
  0x00100000502: 'Again',
  0x00100000503: 'Attn',
  0x00100000504: 'VK_CANCEL',
  0x00100000505: 'Context Menu',
  0x00100000506: 'VK_EXECUTE',
  0x00100000507: 'Find',
  0x00100000508: 'VK_HELP',
  0x00100000509: 'VK_PAUSE',
  0x0010000050a: 'Play',
  0x0010000050b: 'Props',
  0x0010000050c: 'VK_SELECT',
  0x0010000050d: 'Zoom In',
  0x0010000050e: 'Zoom Out',
  0x00100000601: 'Brightness Down',
  0x00100000602: 'Brightness Up',
  0x00100000603: 'Camera',
  0x00100000604: 'Eject',
  0x00100000605: 'Log Off',
  0x00100000606: 'Power',
  0x00100000607: 'Power Off',
  0x00100000608: 'VK_PRINT',
  0x00100000609: 'Hibernate',
  0x0010000060a: 'Standby',
  0x0010000060b: 'Wake Up',
  0x00100000701: 'All Candidates',
  0x00100000702: 'Alphanumeric',
  0x00100000703: 'Code Input',
  0x00100000704: 'Compose',
  0x00100000705: 'VK_CONVERT',
  0x00100000706: 'VK_FINAL',
  0x00100000707: 'Group First',
  0x00100000708: 'Group Last',
  0x00100000709: 'Group Next',
  0x0010000070a: 'Group Previous',
  0x0010000070b: 'Mode Change',
  0x0010000070c: 'Next Candidate',
  0x0010000070d: 'Non Convert',
  0x0010000070e: 'Previous Candidate',
  0x0010000070f: 'Process',
  0x00100000710: 'Single Candidate',
  0x00100000711: 'VK_HANGUL',
  0x00100000712: 'VK_HANJA',
  0x00100000713: 'VK_JUNJA',
  0x00100000714: 'Eisu',
  0x00100000715: 'Hankaku',
  0x00100000716: 'Hiragana',
  0x00100000717: 'Hiragana Katakana',
  0x00100000718: 'VK_KANA',
  0x00100000719: 'VK_KANJI',
  0x0010000071a: 'Katakana',
  0x0010000071b: 'Romaji',
  0x0010000071c: 'Zenkaku',
  0x0010000071d: 'Zenkaku Hankaku',
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
  0x0010000080d: 'VK_F13',
  0x0010000080e: 'VK_F14',
  0x0010000080f: 'VK_F15',
  0x00100000810: 'VK_F16',
  0x00100000811: 'VK_F17',
  0x00100000812: 'VK_F18',
  0x00100000813: 'VK_F19',
  0x00100000814: 'VK_F20',
  0x00100000815: 'VK_F21',
  0x00100000816: 'VK_F22',
  0x00100000817: 'VK_F23',
  0x00100000818: 'VK_F24',
  0x00100000901: 'Soft 1',
  0x00100000902: 'Soft 2',
  0x00100000903: 'Soft 3',
  0x00100000904: 'Soft 4',
  0x00100000905: 'Soft 5',
  0x00100000906: 'Soft 6',
  0x00100000907: 'Soft 7',
  0x00100000908: 'Soft 8',
  0x00100000a01: 'Close',
  0x00100000a02: 'Mail Forward',
  0x00100000a03: 'Mail Reply',
  0x00100000a04: 'Mail Send',
  0x00100000a05: 'Media Play Pause',
  0x00100000a07: 'Media Stop',
  0x00100000a08: 'Media Track Next',
  0x00100000a09: 'Media Track Previous',
  0x00100000a0a: 'New',
  0x00100000a0b: 'Open',
  0x00100000a0c: 'Print',
  0x00100000a0d: 'Save',
  0x00100000a0e: 'Spell Check',
  0x00100000a0f: 'Audio Volume Down',
  0x00100000a10: 'Audio Volume Up',
  0x00100000a11: 'Audio Volume Mute',
  0x00100000b01: 'Launch Application 2',
  0x00100000b02: 'Launch Calendar',
  0x00100000b03: 'Launch Mail',
  0x00100000b04: 'Launch Media Player',
  0x00100000b05: 'Launch Music Player',
  0x00100000b06: 'Launch Application 1',
  0x00100000b07: 'Launch Screen Saver',
  0x00100000b08: 'Launch Spreadsheet',
  0x00100000b09: 'Launch Web Browser',
  0x00100000b0a: 'Launch Web Cam',
  0x00100000b0b: 'Launch Word Processor',
  0x00100000b0c: 'Launch Contacts',
  0x00100000b0d: 'Launch Phone',
  0x00100000b0e: 'Launch Assistant',
  0x00100000b0f: 'Launch Control Panel',
  0x00100000c01: 'Browser Back',
  0x00100000c02: 'Browser Favorites',
  0x00100000c03: 'Browser Forward',
  0x00100000c04: 'Browser Home',
  0x00100000c05: 'Browser Refresh',
  0x00100000c06: 'Browser Search',
  0x00100000c07: 'Browser Stop',
  0x00100000d01: 'Audio Balance Left',
  0x00100000d02: 'Audio Balance Right',
  0x00100000d03: 'Audio Bass Boost Down',
  0x00100000d04: 'Audio Bass Boost Up',
  0x00100000d05: 'Audio Fader Front',
  0x00100000d06: 'Audio Fader Rear',
  0x00100000d07: 'Audio Surround Mode Next',
  0x00100000d08: 'AVR Input',
  0x00100000d09: 'AVR Power',
  0x00100000d0a: 'Channel Down',
  0x00100000d0b: 'Channel Up',
  0x00100000d0c: 'Color F0 Red',
  0x00100000d0d: 'Color F1 Green',
  0x00100000d0e: 'Color F2 Yellow',
  0x00100000d0f: 'Color F3 Blue',
  0x00100000d10: 'Color F4 Grey',
  0x00100000d11: 'Color F5 Brown',
  0x00100000d12: 'Closed Caption Toggle',
  0x00100000d13: 'Dimmer',
  0x00100000d14: 'Display Swap',
  0x00100000d15: 'Exit',
  0x00100000d16: 'Favorite Clear 0',
  0x00100000d17: 'Favorite Clear 1',
  0x00100000d18: 'Favorite Clear 2',
  0x00100000d19: 'Favorite Clear 3',
  0x00100000d1a: 'Favorite Recall 0',
  0x00100000d1b: 'Favorite Recall 1',
  0x00100000d1c: 'Favorite Recall 2',
  0x00100000d1d: 'Favorite Recall 3',
  0x00100000d1e: 'Favorite Store 0',
  0x00100000d1f: 'Favorite Store 1',
  0x00100000d20: 'Favorite Store 2',
  0x00100000d21: 'Favorite Store 3',
  0x00100000d22: 'Guide',
  0x00100000d23: 'Guide Next Day',
  0x00100000d24: 'Guide Previous Day',
  0x00100000d25: 'Info',
  0x00100000d26: 'Instant Replay',
  0x00100000d27: 'Link',
  0x00100000d28: 'List Program',
  0x00100000d29: 'Live Content',
  0x00100000d2a: 'Lock',
  0x00100000d2b: 'Apps',
  0x00100000d2c: 'Media Fast Forward',
  0x00100000d2d: 'Media Last',
  0x00100000d2e: 'Media Pause',
  0x00100000d2f: 'Media Play',
  0x00100000d30: 'Media Record',
  0x00100000d31: 'Media Rewind',
  0x00100000d32: 'Media Skip',
  0x00100000d33: 'Next Favorite Channel',
  0x00100000d34: 'Next User Profile',
  0x00100000d35: 'On Demand',
  0x00100000d36: 'P In P Down',
  0x00100000d37: 'P In P Move',
  0x00100000d38: 'P In P Toggle',
  0x00100000d39: 'P In P Up',
  0x00100000d3a: 'Play Speed Down',
  0x00100000d3b: 'Play Speed Reset',
  0x00100000d3c: 'Play Speed Up',
  0x00100000d3d: 'Random Toggle',
  0x00100000d3e: 'Rc Low Battery',
  0x00100000d3f: 'Record Speed Next',
  0x00100000d40: 'Rf Bypass',
  0x00100000d41: 'Scan Channels Toggle',
  0x00100000d42: 'Screen Mode Next',
  0x00100000d43: 'Settings',
  0x00100000d44: 'Split Screen Toggle',
  0x00100000d45: 'STB Input',
  0x00100000d46: 'STB Power',
  0x00100000d47: 'Subtitle',
  0x00100000d48: 'Teletext',
  0x00100000d49: 'TV',
  0x00100000d4a: 'TV Input',
  0x00100000d4b: 'TV Power',
  0x00100000d4c: 'Video Mode Next',
  0x00100000d4d: 'Wink',
  0x00100000d4e: 'Zoom Toggle',
  0x00100000d4f: 'DVR',
  0x00100000d50: 'Media Audio Track',
  0x00100000d51: 'Media Skip Backward',
  0x00100000d52: 'Media Skip Forward',
  0x00100000d53: 'Media Step Backward',
  0x00100000d54: 'Media Step Forward',
  0x00100000d55: 'Media Top Menu',
  0x00100000d56: 'Navigate In',
  0x00100000d57: 'Navigate Next',
  0x00100000d58: 'Navigate Out',
  0x00100000d59: 'Navigate Previous',
  0x00100000d5a: 'Pairing',
  0x00100000d5b: 'Media Close',
  0x00100000e02: 'Audio Bass Boost Toggle',
  0x00100000e04: 'Audio Treble Down',
  0x00100000e05: 'Audio Treble Up',
  0x00100000e06: 'Microphone Toggle',
  0x00100000e07: 'Microphone Volume Down',
  0x00100000e08: 'Microphone Volume Up',
  0x00100000e09: 'Microphone Volume Mute',
  0x00100000f01: 'Speech Correction List',
  0x00100000f02: 'Speech Input Toggle',
  0x00100001001: 'App Switch',
  0x00100001002: 'Call',
  0x00100001003: 'Camera Focus',
  0x00100001004: 'End Call',
  0x00100001005: 'Go Back',
  0x00100001006: 'Go Home',
  0x00100001007: 'Headset Hook',
  0x00100001008: 'Last Number Redial',
  0x00100001009: 'Notification',
  0x0010000100a: 'Manner Mode',
  0x0010000100b: 'Voice Dial',
  0x00100001101: 'TV 3 D Mode',
  0x00100001102: 'TV Antenna Cable',
  0x00100001103: 'TV Audio Description',
  0x00100001104: 'TV Audio Description Mix Down',
  0x00100001105: 'TV Audio Description Mix Up',
  0x00100001106: 'TV Contents Menu',
  0x00100001107: 'TV Data Service',
  0x00100001108: 'TV Input Component 1',
  0x00100001109: 'TV Input Component 2',
  0x0010000110a: 'TV Input Composite 1',
  0x0010000110b: 'TV Input Composite 2',
  0x0010000110c: 'TV Input HDMI 1',
  0x0010000110d: 'TV Input HDMI 2',
  0x0010000110e: 'TV Input HDMI 3',
  0x0010000110f: 'TV Input HDMI 4',
  0x00100001110: 'TV Input VGA 1',
  0x00100001111: 'TV Media Context',
  0x00100001112: 'TV Network',
  0x00100001113: 'TV Number Entry',
  0x00100001114: 'TV Radio Service',
  0x00100001115: 'TV Satellite',
  0x00100001116: 'TV Satellite BS',
  0x00100001117: 'TV Satellite CS',
  0x00100001118: 'TV Satellite Toggle',
  0x00100001119: 'TV Terrestrial Analog',
  0x0010000111a: 'TV Terrestrial Digital',
  0x0010000111b: 'TV Timer',
  0x00100001201: 'Key 11',
  0x00100001202: 'Key 12',
  0x00200000000: 'Suspend',
  0x00200000001: 'Resume',
  0x00200000002: 'VK_SLEEP',
  0x00200000003: 'Abort',
  0x00200000010: 'Lang 1',
  0x00200000011: 'Lang 2',
  0x00200000012: 'Lang 3',
  0x00200000013: 'Lang 4',
  0x00200000014: 'Lang 5',
  0x00200000020: 'Intl Backslash',
  0x00200000021: 'Intl Ro',
  0x00200000022: 'Intl Yen',
  0x00200000100: 'VK_CONTROL',
  0x00200000101: 'RControl',
  0x00200000102: 'VK_SHIFT',
  0x00200000103: 'RShift',
  0x00200000104: 'VK_MENU',
  0x00200000105: 'RAlt',
  0x00200000106: 'Meta Left',
  0x00200000107: 'Meta Right',
  0x002000001f0: 'VK_CONTROL',
  0x002000001f2: 'VK_SHIFT',
  0x002000001f4: 'VK_MENU',
  0x002000001f6: 'Meta',
  0x0020000020d: 'Numpad Enter',
  0x00200000228: 'Numpad Paren Left',
  0x00200000229: 'Numpad Paren Right',
  0x0020000022a: 'VK_MULTIPLY',
  0x0020000022b: 'VK_ADD',
  0x0020000022c: 'Numpad Comma',
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
  0x0020000023d: 'Numpad Equal',
  0x00200000301: 'Game Button 1',
  0x00200000302: 'Game Button 2',
  0x00200000303: 'Game Button 3',
  0x00200000304: 'Game Button 4',
  0x00200000305: 'Game Button 5',
  0x00200000306: 'Game Button 6',
  0x00200000307: 'Game Button 7',
  0x00200000308: 'Game Button 8',
  0x00200000309: 'Game Button 9',
  0x0020000030a: 'Game Button 10',
  0x0020000030b: 'Game Button 11',
  0x0020000030c: 'Game Button 12',
  0x0020000030d: 'Game Button 13',
  0x0020000030e: 'Game Button 14',
  0x0020000030f: 'Game Button 15',
  0x00200000310: 'Game Button 16',
  0x00200000311: 'Game Button A',
  0x00200000312: 'Game Button B',
  0x00200000313: 'Game Button C',
  0x00200000314: 'Game Button Left 1',
  0x00200000315: 'Game Button Left 2',
  0x00200000316: 'Game Button Mode',
  0x00200000317: 'Game Button Right 1',
  0x00200000318: 'Game Button Right 2',
  0x00200000319: 'Game Button Select',
  0x0020000031a: 'Game Button Start',
  0x0020000031b: 'Game Button Thumb Left',
  0x0020000031c: 'Game Button Thumb Right',
  0x0020000031d: 'Game Button X',
  0x0020000031e: 'Game Button Y',
  0x0020000031f: 'Game Button Z',
};
