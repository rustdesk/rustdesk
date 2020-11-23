import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';
import 'dart:math' as math;
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
  double _xOffset = 0;
  double _yOffset = 0;
  double _scale = 1;
  final FocusNode _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIOverlays([]);
      showLoading('Connecting...');
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

  void interval() {
    var v = MediaQuery.of(context).viewInsets.bottom;
    if (v != _bottom) {
      setState(() {
        _bottom = v;
        if (v < 80) {
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
    print('${MediaQueryData.fromWindow(ui.window).size}');
    print('${MediaQuery.of(context).size}');
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
                        Transform.rotate(
                            angle: 15 * math.pi / 180,
                            child: IconButton(
                              color: Colors.white,
                              icon: Icon(Icons.flash_on),
                              onPressed: () {
                                showActions(context);
                              },
                            )),
                        IconButton(
                          color: Colors.white,
                          icon: Icon(Icons.tv),
                          onPressed: () {
                            showOptions(context);
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
              onLongPressStart: (details) {
                var x = details.globalPosition.dx;
                var y = details.globalPosition.dy;
                print('long press');
                () async {
                  var value = await showMenu(
                    context: context,
                    position:
                        RelativeRect.fromLTRB(x + 20, y + 20, x + 20, y + 20),
                    items: [
                      PopupMenuItem<String>(child: Text('Test'), value: 'mode'),
                    ],
                    elevation: 8.0,
                  );
                  if (value == 'mode') {}
                }();
              },
              onDoubleTap: () {
                print('double tap');
              },
              onTap: () {
                print('tap');
              },
              onScaleStart: (details) {
                _scale = 1;
                _xOffset = details.focalPoint.dx;
                _yOffset = details.focalPoint.dy;
                FFI.canvasModel.startPan();
              },
              onScaleUpdate: (details) {
                var scale = details.scale;
                if (scale == 1) {
                  var x = details.focalPoint.dx;
                  var y = details.focalPoint.dy;
                  var dx = x - _xOffset;
                  var dy = y - _yOffset;
                  FFI.canvasModel.updateOffset(dx, dy);
                  _xOffset = x;
                  _yOffset = y;
                } else {
                  FFI.canvasModel.updateScale(scale / _scale);
                  _scale = scale;
                }
              },
              child: FlutterEasyLoading(
                child: Container(
                    color: MyTheme.canvasColor,
                    child: Stack(children: [
                      ImagePaint(),
                      CursorPaint(),
                      SizedBox(
                        width: 0,
                        height: 0,
                        child: _bottom < 100
                            ? Container()
                            : TextField(
                                textInputAction: TextInputAction.newline,
                                autocorrect: false,
                                enableSuggestions: false,
                                focusNode: _focusNode,
                                maxLines: null,
                                keyboardType: TextInputType.multiline,
                                onChanged: (x) => print('$x'),
                              ),
                      ),
                    ])),
              )),
        ));
  }

  void close() {
    msgbox('', 'Close', 'Are you sure to close the connection?', context);
  }
}

class ImagePaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    final c = Provider.of<CanvasModel>(context);
    var s = c.scale;
    return CustomPaint(
      painter:
          new ImagePainter(image: m.image, x: c.x / s, y: c.y / s, scale: s),
    );
  }
}

class CursorPaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    var s = c.scale;
    return CustomPaint(
      painter: new ImagePainter(
          image: m.image,
          x: m.x * s - m.hotx + c.x,
          y: m.y * s - m.hoty + c.y,
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
                  showLoading('Logging in...');
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
  showAlertDialog(
      context,
      (setState) => Tuple3(
          null,
          Column(mainAxisSize: MainAxisSize.min, children: [
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
            Divider(color: Colors.black),
            CheckboxListTile(
                value: showRemoteCursor,
                onChanged: (v) {
                  setState(() {
                    showRemoteCursor = v;
                    FFI.setByName('toggle_option', 'show-remote-cursor');
                  });
                },
                title: Text('Show remote cursor')),
            CheckboxListTile(
                value: lockAfterSessionEnd,
                onChanged: (v) {
                  setState(() {
                    lockAfterSessionEnd = v;
                    FFI.setByName('toggle_option', 'lock-after-session-end');
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
  showAlertDialog(
      context,
      (setState) => Tuple3(
          null,
          Column(mainAxisSize: MainAxisSize.min, children: [
            ListTile(
              onTap: () {
                Navigator.pop(context);
                FFI.setByName('ctrl_alt_del');
              },
              title: Text('Insert Ctrl + Alt + Del'),
            ),
            ListTile(
              onTap: () {
                Navigator.pop(context);
                FFI.setByName('lock_screen');
              },
              title: Text('Insert Lock'),
            ),
          ]),
          null),
      () async => true,
      true,
      0);
}
