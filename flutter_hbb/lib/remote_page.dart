import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'common.dart';
import 'package:flutter/services.dart';

class RemotePage extends StatefulWidget {
  RemotePage({Key key, this.id}) : super(key: key);

  final String id;

  @override
  _RemotePageState createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  FfiModel _ffi;

  @override
  Widget build(BuildContext context) {
    _ffi = Provider.of<FfiModel>(context);
    _ffi.connect(widget.id);
    // https://stackoverflow.com/questions/46640116/make-flutter-application-fullscreen
    SystemChrome.setEnabledSystemUIOverlays([]);
  }
}
