import 'package:flutter/material.dart';
import 'common.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;

class RemotePage extends StatefulWidget {
  RemotePage({Key key, this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  @override
  void initState() {
    super.initState();
    FFI.connect(widget.id);
  }

  @override
  Widget build(BuildContext context) {
    // https://stackoverflow.com/questions/46640116/make-flutter-application-fullscreen
    SystemChrome.setEnabledSystemUIOverlays([]);
    return CustomPaint(
      painter: new ImageEditor(image: null),
    );
  }
}

class ImageEditor extends CustomPainter {
  ImageEditor({
    this.image,
  });

  ui.Image image;

  @override
  void paint(Canvas canvas, Size size) {
    if (image = null) return;
    canvas.drawImage(image, new Offset(0.0, 0.0), new Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return false;
  }
}
