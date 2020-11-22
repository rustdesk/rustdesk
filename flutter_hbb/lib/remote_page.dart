import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';
import 'dart:math' as math;
import 'package:tuple/tuple.dart';
import 'common.dart';
import 'model.dart';

class RemotePage extends StatefulWidget {
  RemotePage({Key key, this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

// https://github.com/hanxu317317/flutter_plan_demo/blob/master/lib/src/enter.dart
class _RemotePageState extends State<RemotePage> {
  Timer _interval;
  bool _showBar = true;
  double _bottom = 0;
  bool _pan = false;
  final FocusNode _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      // https://stackoverflow.com/questions/46640116/make-flutter-application-fullscreen
      SystemChrome.setEnabledSystemUIOverlays([]);
      showLoading('Connecting...');
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
    FFI.close();
    _interval.cancel();
    dismissLoading();
    SystemChrome.setEnabledSystemUIOverlays(SystemUiOverlay.values);
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

  void _handleKeyEvent(RawKeyEvent event) {
    print('$event');
  }

  @override
  Widget build(BuildContext context) {
    // Size size = MediaQueryData.fromWindow(ui.window).size;
    // MediaQuery.of(context).size.height;
    EasyLoading.instance.loadingStyle = EasyLoadingStyle.light;
    return RawKeyboardListener(
        focusNode: _focusNode,
        onKey: _handleKeyEvent,
        child: WillPopScope(
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
                                    SystemChannels.textInput
                                        .invokeMethod('TextInput.show');
                                    _focusNode.requestFocus();
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
                              Container(
                                  color: _pan ? Colors.blue[500] : null,
                                  child: IconButton(
                                    color: Colors.white,
                                    icon: Icon(Icons.pan_tool),
                                    onPressed: () {
                                      setState(() => _pan = !_pan);
                                    },
                                  ))
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
                body: FlutterEasyLoading(
                    child: Container(
                  color: MyTheme.canvasColor,
                  child: RawGestureDetector(
                      gestures: {
                        MultiTouchGestureRecognizer:
                            GestureRecognizerFactoryWithHandlers<
                                MultiTouchGestureRecognizer>(
                          () => MultiTouchGestureRecognizer(),
                          (MultiTouchGestureRecognizer instance) {
                            instance.onMultiTap = (
                              touchCount,
                              addOrRemove,
                            ) =>
                                print('$touchCount, $addOrRemove');
                          },
                        ),
                      },
                      child: GestureDetector(
                        onTap: () {
                          if (_pan) return;
                        },
                        onDoubleTap: () {
                          if (_pan) return;
                        },
                        onLongPress: () {
                          if (_pan) return;
                        },
                        child: InteractiveViewer(
                            constrained: false,
                            panEnabled: _pan,
                            onInteractionUpdate: (details) {
                              // print('$details');
                            },
                            onInteractionStart: (s) {
                              print('$s');
                            },
                            onInteractionEnd: (x) {
                              print('$x');
                            },
                            child: Stack(children: [
                              ImagePaint(),
                              CursorPaint(),
                            ])),
                      )),
                )))));
  }

  void close() {
    msgbox('', 'Close', 'Are you sure to close the connection?', context);
  }
}

class ImagePaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    return CustomPaint(
      painter: new ImagePainter(image: m.image, x: 0, y: 0),
    );
  }
}

class CursorPaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    return CustomPaint(
      painter: new ImagePainter(image: m.image, x: m.x, y: m.y),
    );
  }
}

class ImagePainter extends CustomPainter {
  ImagePainter({
    this.image,
    this.x,
    this.y,
  });

  ui.Image image;
  double x;
  double y;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
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
                Navigator.pop(context);
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

class MultiTouchGestureRecognizer extends MultiTapGestureRecognizer {
  MultiTouchGestureRecognizerCallback onMultiTap;
  var numberOfTouches = 0;

  MultiTouchGestureRecognizer() {
    super.onTapDown = (pointer, details) => addTouch(pointer, details);
    super.onTapUp = (pointer, details) => removeTouch(pointer, details);
    super.onTapCancel = (pointer) => cancelTouch(pointer);
    super.onTap = (pointer) => captureDefaultTap(pointer);
  }

  void addTouch(int pointer, TapDownDetails details) {
    numberOfTouches++;
    onMultiTap(numberOfTouches, true);
  }

  void removeTouch(int pointer, TapUpDetails details) {
    numberOfTouches--;
    onMultiTap(numberOfTouches, false);
  }

  void cancelTouch(int pointer) {
    numberOfTouches = 0;
  }

  void captureDefaultTap(int pointer) {}
}

typedef MultiTouchGestureRecognizerCallback = void Function(
    int touchCount, bool addOrRemove);
