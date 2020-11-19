import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:async';
import 'dart:math' as math;
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
  bool _show_bar = true;

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
    return Scaffold(
        backgroundColor: MyTheme.grayBg,
        floatingActionButton: _show_bar
            ? null
            : FloatingActionButton(
                mini: true,
                child: Icon(Icons.expand_less),
                backgroundColor: MyTheme.accent50,
                onPressed: () {
                  setState(() => _show_bar = !_show_bar);
                }),
        bottomNavigationBar: _show_bar
            ? BottomAppBar(
                color: MyTheme.accent,
                child: Row(
                  mainAxisSize: MainAxisSize.max,
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: <Widget>[
                    Row(children: [
                      IconButton(
                        color: Colors.white,
                        icon: Icon(Icons.clear),
                        onPressed: () {},
                      ),
                      IconButton(
                        color: Colors.white,
                        icon: Icon(Icons.keyboard),
                        onPressed: () {},
                      ),
                      Transform.rotate(
                          angle: 15 * math.pi / 180,
                          child: IconButton(
                            color: Colors.white,
                            icon: Icon(Icons.flash_on),
                            onPressed: () {},
                          )),
                      IconButton(
                        color: Colors.white,
                        icon: Icon(Icons.tv),
                        onPressed: () {},
                      ),
                      IconButton(
                        color: Colors.white,
                        icon: Icon(Icons.settings),
                        onPressed: () {},
                      )
                    ]),
                    IconButton(
                        color: Colors.white,
                        icon: Icon(Icons.expand_more),
                        onPressed: () {
                          setState(() => _show_bar = !_show_bar);
                        }),
                  ],
                ),
              )
            : null,
        body: FlutterEasyLoading(
          child: InteractiveViewer(
            constrained: false,
            panEnabled: true,
            onInteractionUpdate: (details) {
              print("$details");
            },
            child: Stack(children: [
              ImagePaint(),
              CursorPaint(),
            ]),
          ),
        ));
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
