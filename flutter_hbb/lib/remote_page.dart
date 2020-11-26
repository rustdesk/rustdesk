import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';
import 'package:tuple/tuple.dart';
import 'package:wakelock/wakelock.dart';
import 'common.dart';
import 'model.dart';

class RemotePage extends StatefulWidget {
  RemotePage({Key key, this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  Timer _interval;
  bool _showBar = true;
  double _bottom = 0;
  String _value = '';
  double _xOffset = 0;
  double _yOffset = 0;
  double _scale = 1;
  bool _mouseTools = false;
  var _drag = false;
  var _right = false;
  var _scroll = false;
  var _arrows = false;
  var _more = false;
  var _fn = false;
  final FocusNode _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIOverlays([]);
      showLoading('Connecting...', context);
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
    Wakelock.enable();
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
    FFI.close();
    _interval.cancel();
    dismissLoading();
    SystemChrome.setEnabledSystemUIOverlays(SystemUiOverlay.values);
    Wakelock.disable();
  }

  void resetTool() {
    _scroll = _drag = _right = false;
    FFI.resetModifiers();
  }

  void interval() {
    var v = MediaQuery.of(context).viewInsets.bottom;
    if (v != _bottom) {
      resetTool();
      _value = ' ' * 1000;
      setState(() {
        _bottom = v;
        if (v < 100) {
          SystemChrome.setEnabledSystemUIOverlays([]);
        }
      });
    }
    FFI.ffiModel.update(widget.id, context, handleMsgbox);
  }

  void handleMsgbox(Map<String, dynamic> evt, String id, BuildContext context) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id, context);
    } else if (type == 'input-password') {
      enterPasswordDialog(id, context);
    } else {
      msgbox(type, title, text, context);
    }
  }

  @override
  Widget build(BuildContext context) {
    EasyLoading.instance.loadingStyle = EasyLoadingStyle.light;
    return WillPopScope(
        onWillPop: () async {
          close();
          return false;
        },
        child: Scaffold(
          floatingActionButton: _showBar
              ? null
              : FloatingActionButton(
                  mini: true,
                  child: Icon(Icons.expand_less),
                  backgroundColor: MyTheme.accent50,
                  onPressed: () {
                    setState(() => _showBar = !_showBar);
                  }),
          bottomNavigationBar: _showBar
              ? BottomAppBar(
                  elevation: 10,
                  color: MyTheme.accent,
                  child: Row(
                    mainAxisSize: MainAxisSize.max,
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: <Widget>[
                      Row(children: [
                        IconButton(
                          color: Colors.white,
                          icon: Icon(Icons.clear),
                          onPressed: () {
                            close();
                          },
                        ),
                        IconButton(
                            color: Colors.white,
                            icon: Icon(Icons.keyboard),
                            onPressed: () {
                              SystemChrome.setEnabledSystemUIOverlays(
                                  SystemUiOverlay.values);
                              _focusNode.requestFocus();
                              SystemChannels.textInput
                                  .invokeMethod('TextInput.show');
                            }),
                        IconButton(
                          color: Colors.white,
                          icon: Icon(Icons.tv),
                          onPressed: () {
                            showOptions(context);
                          },
                        ),
                        Container(
                            color: _mouseTools ? Colors.blue[500] : null,
                            child: IconButton(
                              color: Colors.white,
                              icon: Icon(Icons.mouse),
                              onPressed: () {
                                setState(() {
                                  _mouseTools = !_mouseTools;
                                  resetTool();
                                });
                              },
                            )),
                        IconButton(
                          color: Colors.white,
                          icon: Icon(Icons.more_vert),
                          onPressed: () {
                            showActions(context);
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
                )
              : null,
          body: GestureDetector(
              onTap: () {
                if (_drag || _scroll) return;
                FFI.tap(_right);
              },
              onLongPressStart: (_) {
                if (_drag) {
                  // case: to show password on windows
                  FFI.sendMouse('down', 'left');
                }
              },
              onLongPressEnd: (_) {
                if (_drag) {
                  FFI.sendMouse('up', 'left');
                }
              },
              onScaleStart: (details) {
                _scale = 1;
                _xOffset = details.focalPoint.dx;
                _yOffset = details.focalPoint.dy;
                if (_drag) {
                  FFI.sendMouse('down', 'left');
                }
              },
              onScaleUpdate: (details) {
                var scale = details.scale;
                if (scale == 1) {
                  var x = details.focalPoint.dx;
                  var y = details.focalPoint.dy;
                  var dx = x - _xOffset;
                  var dy = y - _yOffset;
                  if (_scroll) {
                    FFI.scroll(-dy);
                  } else {
                    FFI.cursorModel.updatePan(dx, dy);
                  }
                  _xOffset = x;
                  _yOffset = y;
                } else if (!_drag && !_scroll) {
                  FFI.canvasModel.updateScale(scale / _scale);
                  _scale = scale;
                }
              },
              onScaleEnd: (_) {
                if (_drag) {
                  FFI.sendMouse('up', 'left');
                }
              },
              child: FlutterEasyLoading(
                child: Container(
                    color: MyTheme.canvasColor,
                    child: Stack(children: [
                      ImagePaint(),
                      CursorPaint(),
                      getHelpTools(),
                      SizedBox(
                        width: 0,
                        height: 0,
                        child: _bottom < 100
                            ? Container()
                            : TextFormField(
                                textInputAction: TextInputAction.newline,
                                autocorrect: false,
                                enableSuggestions: false,
                                focusNode: _focusNode,
                                maxLines: null,
                                initialValue:
                                    _value, // trick way to make backspace work always
                                keyboardType: TextInputType.multiline,
                                onChanged: (x) {
                                  var char = x[x.length - 1];
                                  if (x.length <= _value.length) {
                                    char = 'VK_BACK';
                                  } else if (char == '\n') {
                                    char = 'VK_RETURN';
                                  }
                                  if (char != '' && char != null)
                                    FFI.inputKey(char);
                                  _value = x;
                                },
                              ),
                      ),
                    ])),
              )),
        ));
  }

  void close() {
    msgbox('', 'Close', 'Are you sure to close the connection?', context);
  }

  Widget getHelpTools() {
    final keyboard = _bottom >= 100;
    if (!_mouseTools && !keyboard) {
      return SizedBox();
    }
    var wrap =
        (String text, void Function() onPressed, [bool active, IconData icon]) {
      return ButtonTheme(
          padding: EdgeInsets.symmetric(
              vertical: 6, horizontal: 12), //adds padding inside the button
          materialTapTargetSize: MaterialTapTargetSize
              .shrinkWrap, //limits the touch area to the button area
          minWidth: 0, //wraps child's width
          height: 0,
          child: FlatButton(
              splashColor: Colors.black,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(5.0),
              ),
              color: active == true ? MyTheme.accent50 : null,
              child: icon != null
                  ? Icon(icon, color: Colors.white)
                  : Text(text,
                      style: TextStyle(color: Colors.white, fontSize: 11)),
              onPressed: onPressed));
    };
    final mouse = <Widget>[
      wrap('Drag', () {
        setState(() {
          _drag = !_drag;
          if (_drag) {
            _scroll = false;
            _right = false;
          }
        });
      }, _drag),
      wrap('Scroll', () {
        setState(() {
          _scroll = !_scroll;
          if (_drag) {
            _drag = false;
            _right = false;
          }
        });
      }, _scroll),
      wrap('Right', () {
        setState(() {
          _right = !_right;
          if (_drag) {
            _scroll = false;
            _drag = false;
          }
        });
      }, _right)
    ];
    final modifiers = <Widget>[
      wrap('Ctrl', () {
        setState(() => FFI.ctrl = !FFI.ctrl);
      }, FFI.ctrl),
      wrap('Alt', () {
        setState(() => FFI.alt = !FFI.alt);
      }, FFI.alt),
      wrap('Shift', () {
        setState(() => FFI.shift = !FFI.shift);
      }, FFI.shift),
      wrap('Command', () {
        setState(() => FFI.command = !FFI.command);
      }, FFI.command),
    ];
    final keys = <Widget>[
      wrap(
          'Arrows',
          () => setState(() {
                setState(() {
                  _arrows = !_arrows;
                  if (_arrows) {
                    _fn = false;
                    _more = false;
                  }
                });
              }),
          _arrows),
      wrap(
          'Fn',
          () => setState(
                () {
                  _fn = !_fn;
                  if (_fn) {
                    _arrows = false;
                    _more = false;
                  }
                },
              ),
          _fn),
      wrap(
          'More',
          () => setState(
                () {
                  _more = !_more;
                  if (_more) {
                    _arrows = false;
                    _fn = false;
                  }
                },
              ),
          _more),
    ];
    final arrows = <Widget>[
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
      wrap('PeUp', () {
        FFI.inputKey('VK_PRIOR');
      }),
      wrap('PgDown', () {
        FFI.inputKey('VK_NEXT');
      }),
    ];
    return Container(
        color: Color(0x77000000),
        padding: EdgeInsets.only(
            top: keyboard ? 24 : 4, left: 8, right: 8, bottom: 8),
        child: Wrap(
          spacing: 4,
          runSpacing: 4,
          children: <Widget>[SizedBox(width: 9999)] +
              (keyboard
                  ? modifiers +
                      keys +
                      (_arrows ? arrows : []) +
                      (_fn ? fn : []) +
                      (_more ? more : [])
                  : mouse + modifiers),
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
    this.image,
    this.x,
    this.y,
    this.scale,
  });

  ui.Image image;
  double x;
  double y;
  double scale;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    canvas.scale(scale, scale);
    canvas.drawImage(image, new Offset(x, y), new Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}

void enterPasswordDialog(String id, BuildContext context) {
  final controller = TextEditingController();
  var remember = FFI.getByName('remember', id) == 'true';
  showAlertDialog(
      context,
      (setState) => Tuple3(
            Text('Please enter your password'),
            Column(mainAxisSize: MainAxisSize.min, children: [
              PasswordWidget(controller: controller),
              CheckboxListTile(
                controlAffinity: ListTileControlAffinity.leading,
                title: Text(
                  'Remember the password',
                ),
                value: remember,
                onChanged: (v) {
                  setState(() => remember = v);
                },
              ),
            ]),
            [
              FlatButton(
                textColor: MyTheme.accent,
                onPressed: () {
                  Navigator.pop(context);
                  Navigator.pop(context);
                },
                child: Text('Cancel'),
              ),
              FlatButton(
                textColor: MyTheme.accent,
                onPressed: () {
                  var text = controller.text.trim();
                  if (text == '') return;
                  FFI.login(text, remember);
                  showLoading('Logging in...', null);
                  Navigator.pop(context);
                },
                child: Text('OK'),
              ),
            ],
          ));
}

void wrongPasswordDialog(String id, BuildContext context) {
  showAlertDialog(
      context,
      (_) =>
          Tuple3(Text('Wrong Password'), Text('Do you want to enter again?'), [
            FlatButton(
              textColor: MyTheme.accent,
              onPressed: () {
                Navigator.pop(context);
                Navigator.pop(context);
              },
              child: Text('Cancel'),
            ),
            FlatButton(
              textColor: MyTheme.accent,
              onPressed: () {
                enterPasswordDialog(id, context);
              },
              child: Text('Retry'),
            ),
          ]));
}

void showOptions(BuildContext context) {
  var showRemoteCursor =
      FFI.getByName('toggle_option', 'show-remote-cursor') == 'true';
  var lockAfterSessionEnd =
      FFI.getByName('toggle_option', 'lock-after-session-end') == 'true';
  String quality = FFI.getByName('image_quality');
  if (quality == '') quality = 'balanced';
  var displays = <Widget>[];
  final pi = FFI.ffiModel.pi;
  if (pi.displays.length > 1) {
    final cur = pi.currentDisplay;
    final children = <Widget>[];
    for (var i = 0; i < pi.displays.length; ++i)
      children.add(InkWell(
          onTap: () {
            if (i == cur) return;
            FFI.setByName('switch_display', i.toString());
            Navigator.pop(context);
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
    displays.add(Divider(color: MyTheme.border));
  }
  showAlertDialog(
      context,
      (setState) => Tuple3(
          null,
          Column(
              mainAxisSize: MainAxisSize.min,
              children: displays +
                  <Widget>[
                    RadioListTile<String>(
                      controlAffinity: ListTileControlAffinity.trailing,
                      title: const Text('Good image quality'),
                      value: 'best',
                      groupValue: quality,
                      onChanged: (String value) {
                        setState(() {
                          quality = value;
                          FFI.setByName('image_quality', value);
                        });
                      },
                    ),
                    RadioListTile<String>(
                      controlAffinity: ListTileControlAffinity.trailing,
                      title: const Text('Balanced'),
                      value: 'balanced',
                      groupValue: quality,
                      onChanged: (String value) {
                        setState(() {
                          quality = value;
                          FFI.setByName('image_quality', value);
                        });
                      },
                    ),
                    RadioListTile<String>(
                      controlAffinity: ListTileControlAffinity.trailing,
                      title: const Text('Optimize reaction time'),
                      value: 'low',
                      groupValue: quality,
                      onChanged: (String value) {
                        setState(() {
                          quality = value;
                          FFI.setByName('image_quality', value);
                        });
                      },
                    ),
                    Divider(color: MyTheme.border),
                    CheckboxListTile(
                        value: showRemoteCursor,
                        onChanged: (v) {
                          setState(() {
                            showRemoteCursor = v;
                            FFI.setByName(
                                'toggle_option', 'show-remote-cursor');
                          });
                        },
                        title: Text('Show remote cursor')),
                    CheckboxListTile(
                        value: lockAfterSessionEnd,
                        onChanged: (v) {
                          setState(() {
                            lockAfterSessionEnd = v;
                            FFI.setByName(
                                'toggle_option', 'lock-after-session-end');
                          });
                        },
                        title: Text('Lock after session end'))
                  ]),
          null),
      () async => true,
      true,
      0);
}

void showActions(BuildContext context) {
  final size = MediaQuery.of(context).size;
  final x = 120.0;
  final y = size.height;
  () async {
    var value = await showMenu(
      context: context,
      position: RelativeRect.fromLTRB(x, y, x, y),
      items: [
        PopupMenuItem<String>(
            child: Text('Insert Ctrl + Alt + Del'), value: 'cad'),
        PopupMenuItem<String>(child: Text('Insert Lock'), value: 'lock'),
      ],
      elevation: 8,
    );
    if (value == 'cad') {
      FFI.setByName('ctrl_alt_del');
    }
    if (value == 'lock') {
      FFI.setByName('lock_screen');
    }
  }();
}
