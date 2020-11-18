import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'common.dart';
import 'package:flutter/services.dart';
import 'dart:ui' as ui;
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'dart:convert';
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
  PeerInfo _pi = PeerInfo();
  Display _display = Display();
  bool _decoding = false;

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
    _decoding = null;
    SystemChrome.setEnabledSystemUIOverlays(SystemUiOverlay.values);
  }

  void interval() {
    var evt = FFI.popEvent();
    if (evt != null) {
      var name = evt['name'];
      if (name == 'msgbox') {
        handleMsgbox(evt);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt);
      } else if (name == 'switch_display') {
        handleSwitchDisplay(evt);
      } else if (name == 'cursor_data') {
        FFI.cursorModel.updateCursorData(evt);
      } else if (name == 'cursor_id') {
        FFI.cursorModel.updateCursorId(evt);
      } else if (name == 'cursor_position') {
        FFI.cursorModel.updateCursorPosition(evt);
      }
    }
    if (!_decoding) {
      var rgba = FFI.getRgba();
      if (rgba != null) {
        _decoding = true;
        ui.decodeImageFromPixels(
            rgba, _display.width, _display.height, ui.PixelFormat.bgra8888,
            (image) {
          FFI.clearRgbaFrame();
          if (_decoding == null) return;
          _decoding = false;
          FFI.imageModel.update(image);
        });
      }
    }
  }

  void handleSwitchDisplay(Map<String, dynamic> evt) {
    _pi.currentDisplay = int.parse(evt['display']);
    _display.x = int.parse(evt['x']);
    _display.y = int.parse(evt['y']);
    _display.width = int.parse(evt['width']);
    _display.height = int.parse(evt['height']);
    setState(() {});
  }

  void handlePeerInfo(Map<String, dynamic> evt) {
    dismissLoading();
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == "true";
    _pi.currentDisplay = int.parse(evt['current_display']);
    List<dynamic> displays = json.decode(evt['displays']);
    _pi.displays = List<Display>();
    for (int i = 0; i < displays.length; ++i) {
      Map<String, dynamic> d0 = displays[i];
      var d = Display();
      d.x = d0['x'];
      d.y = d0['y'];
      d.width = d0['width'];
      d.height = d0['height'];
      _pi.displays.add(d);
    }
    if (_pi.currentDisplay < _pi.displays.length) {
      _display = _pi.displays[_pi.currentDisplay];
    }
    setState(() {});
  }

  void handleMsgbox(Map<String, dynamic> evt) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(widget.id, context);
    } else if (type == 'input-password') {
      enterPasswordDialog(widget.id, context);
    } else {
      msgbox(type, title, text, context);
    }
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

class Display {
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
}

class PeerInfo {
  String username;
  String hostname;
  String platform;
  bool sasEnabled;
  int currentDisplay;
  List<Display> displays;
}
