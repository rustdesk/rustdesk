import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';
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
    super.dispose();
    FFI.close();
    _interval.cancel();
    dismissLoading();
    SystemChrome.setEnabledSystemUIOverlays(SystemUiOverlay.values);
  }

  void interval() {
    FFI.ffiModel.update(widget.id, context);
  }

  @override
  Widget build(BuildContext context) {
    // Size size = MediaQueryData.fromWindow(ui.window).size;
    // MediaQuery.of(context).size.height;
    return FlutterEasyLoading(
        child: InteractiveViewer(
      constrained: false,
      panEnabled: true,
      onInteractionUpdate: (details) {
        print("$details");
      },
      child: Container(child: ImagePaint(), color: MyTheme.grayBg),
    ));
  }
}

class ImagePaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    return CustomPaint(
      painter: new ImagePainter(image: m.image),
    );
  }
}

class ImagePainter extends CustomPainter {
  ImagePainter({
    this.image,
  });

  ui.Image image;

  @override
  void paint(Canvas canvas, Size size) {
    if (image == null) return;
    canvas.drawImage(image, new Offset(0, 0), new Paint());
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) {
    return oldDelegate != this;
  }
}
