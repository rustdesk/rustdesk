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

  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      showLoading("Connecting...");
      _interval =
          Timer.periodic(Duration(milliseconds: 30), (timer) => interval());
    });
  }

  @override
  void dispose() {
    super.dispose();
    _interval.cancel();
    dismissLoading();
    FFI.close();
  }

  void interval() {
    print(1);
  }

  @override
  Widget build(BuildContext context) {
    // Size size = MediaQueryData.fromWindow(ui.window).size;
    // https://stackoverflow.com/questions/46640116/make-flutter-application-fullscreen
    SystemChrome.setEnabledSystemUIOverlays([]);
    return FlutterEasyLoading(
        child: CustomPaint(
      painter: new ImageEditor(image: null),
    ));
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
