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
  final FocusNode _focusNode = FocusNode();
  var _showEdit = false;
  var _touchMode = false;

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
  }

  @override
  void dispose() {
    _focusNode.dispose();
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
        SystemChannels.textInput.invokeMethod('TextInput.show');
        _focusNode.requestFocus();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final pi = Provider.of<FfiModel>(context).pi;
    final hideKeyboard = isKeyboardShown() && _showEdit;
    final showActionButton = !_showBar || hideKeyboard;
    return WillPopScope(
      onWillPop: () async {
        clientClose();
        return false;
      },
      child: Scaffold(
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
                    child: isDesktop
                        ? getBodyForDesktopWithListener()
                        : SafeArea(child: getBodyForMobileWithGesture()));
              })
            ],
          )),
    );
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
          if (_touchMode) {
            FFI.tap(MouseButtons.right);
          }
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
        onHoldDragCancel: () {
          if (!_touchMode) {
            FFI.sendMouse('up', MouseButtons.left);
          }
        },
        onOneFingerPanStart: (d) {
          if (_touchMode) {
            debugPrint("_touchMode , onOneFingerPanStart");
            FFI.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
            FFI.sendMouse('down', MouseButtons.left);
          } else {
            FFI.sendMouse('up', MouseButtons.left);
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
                    focusNode: _focusNode,
                    maxLines: null,
                    initialValue: _value,
                    // trick way to make backspace work always
                    keyboardType: TextInputType.multiline,
                    onChanged: handleInput,
                  ),
          ),
        ]));
  }

  Widget getBodyForDesktopWithListener() {
    final keyboard = FFI.ffiModel.permissions['keyboard'] != false;
    var paints = <Widget>[ImagePaint()];
    if (keyboard ||
        FFI.getByName('toggle_option', 'show-remote-cursor') == 'true') {
      paints.add(CursorPaint());
    }
    return MouseRegion(
        cursor: keyboard ? SystemMouseCursors.none : MouseCursor.defer,
        // TODO old null // still laggy, set cursor directly for web is better
        onEnter: (event) {
          print('enter');
          FFI.listenToMouse(true);
        },
        onExit: (event) {
          print('exit');
          FFI.listenToMouse(false);
        },
        child: Container(
            color: MyTheme.canvasColor, child: Stack(children: paints)));
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
              (isDesktop
                  ? <Widget>[
                      getRadio('Original', 'original', viewStyle, setViewStyle),
                      getRadio('Shrink', 'shrink', viewStyle, setViewStyle),
                      getRadio('Stretch', 'stretch', viewStyle, setViewStyle),
                      Divider(color: MyTheme.border),
                    ]
                  : []) +
              <Widget>[
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
