import 'package:flutter/material.dart';
import 'common.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';

class RemotePage extends StatefulWidget {
  RemotePage({Key key, this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

// https://github.com/hanxu317317/flutter_plan_demo/blob/master/lib/src/enter.dart
class _RemotePageState extends State<RemotePage> {
  Timer _interval;
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
  ui.Image image;

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      showLoading('Connecting...');
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
  }

  @override
  void dispose() {
    super.dispose();
    FFI.close();
    _interval.cancel();
    dismissLoading();
  }

  void interval() {
    var evt = FFI.popEvent();
    if (evt == null) return;
    var name = evt['name'];
    if (name == 'msgbox') {
      handleMsgbox(evt);
    }
    var rgba = FFI.getRgba();
    if (rgba != null) {
      ui.decodeImageFromPixels(rgba, width, height, ui.PixelFormat.rgba8888,
          (_image) {
        setState(() {
          image = _image;
        });
      });
    }
  }

  void handleMsgbox(evt) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 'error') {
    } else if (type == 're-input-password') {
      wrongPasswordDialog(widget.id, context);
    } else if (type == 'input-password') {
      enterPasswordDialog(widget.id, context);
    }
  }

  @override
  Widget build(BuildContext context) {
    // Size size = MediaQueryData.fromWindow(ui.window).size;
    // MediaQuery.of(context).size.height;
    // https://stackoverflow.com/questions/46640116/make-flutter-application-fullscreen
    SystemChrome.setEnabledSystemUIOverlays([]);
    return FlutterEasyLoading(
        child: GestureDetector(
            child: CustomPaint(
              painter: new ImageEditor(image: image),
            ),
            onPanStart: (DragDownDetails) {
              print('onPanStart $DragDownDetails');
              // hero.moveTo(DragDownDetails.globalPosition.dx, DragDownDetails.globalPosition.dy);
            },
            onPanUpdate: (DragDownDetails) {
              print('onPanUpdate $DragDownDetails');
              // hero.moveTo(DragDownDetails.globalPosition.dx, DragDownDetails.globalPosition.dy);
            }));
  }
}

class ImageEditor extends CustomPainter {
  ImageEditor({
    this.image,
  });

  ui.Image image;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    canvas.drawImage(image, new Offset(0.0, 0.0), new Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}
