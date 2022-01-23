import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/model.dart';

import 'common.dart';

class ServerPage extends StatefulWidget {
  @override
  _ServerPageState createState() => _ServerPageState();
}

class _ServerPageState extends State<ServerPage> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
        backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          centerTitle: true,
          title: const Text("Share My Screen"),
        ),
        body: SingleChildScrollView(
          child: Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              children: [
                ServerInfo(),
                PermissionChecker(),
              ],
            ),
          ),
        ));
  }
}

class ServerInfo extends StatefulWidget {
  @override
  _ServerInfoState createState() => _ServerInfoState();
}

class _ServerInfoState extends State<ServerInfo> {
  var _passwdShow = true;

  // TODO set ID / PASSWORD
  var _serverId = "";
  var _serverPasswd = "";

  @override
  void initState() {
    super.initState();
    _serverId = FFI.getByName("server_id");
    _serverPasswd = FFI.getByName("server_password");
  }

  @override
  Widget build(BuildContext context) {
    return myCard(Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        TextFormField(
          readOnly: true,
          style: TextStyle(
              fontSize: 25.0,
              fontWeight: FontWeight.bold,
              color: MyTheme.accent),
          initialValue: _serverId,
          decoration: InputDecoration(
            icon: const Icon(Icons.perm_identity),
            labelText: '服务ID',
            labelStyle:
                TextStyle(fontWeight: FontWeight.bold, color: MyTheme.accent50),
          ),
          onSaved: (String value) {},
        ),
        TextFormField(
          readOnly: true,
          obscureText: _passwdShow,
          style: TextStyle(
              fontSize: 25.0,
              fontWeight: FontWeight.bold,
              color: MyTheme.accent),
          initialValue: _serverPasswd,
          decoration: InputDecoration(
              icon: const Icon(Icons.lock),
              labelText: '密码',
              labelStyle: TextStyle(
                  fontWeight: FontWeight.bold, color: MyTheme.accent50),
              suffix: IconButton(
                  icon: Icon(Icons.visibility),
                  onPressed: () {
                    debugPrint("icon btn");
                    setState(() {
                      _passwdShow = !_passwdShow;
                    });
                  })),
          onSaved: (String value) {},
        ),
      ],
    ));
  }
}

class PermissionChecker extends StatefulWidget {
  @override
  _PermissionCheckerState createState() => _PermissionCheckerState();
}

class _PermissionCheckerState extends State<PermissionChecker> {
  static const toAndroidChannel = MethodChannel("mChannel");

  var videoOk = false;
  var inputOk = false;
  var audioOk = false;

  @override
  Widget build(BuildContext context) {
    return myCard(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        cardTitle("权限列表"),
        PermissionRow("视频权限", videoOk, _toAndroidGetPer),
        const Divider(height: 0),
        PermissionRow("音频权限", videoOk, () => {debugPrint("获取视频权限")}),
        const Divider(height: 0),
        PermissionRow("输入权限", inputOk, _toAndroidCheckInput),
        const Divider(),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceAround,
          children: [
            TextButton.icon(
                icon: Icon(Icons.play_arrow),
                onPressed: _toAndroidStartSer,
                label: Text("Start")),
            TextButton.icon(
                icon: Icon(Icons.stop),
                onPressed: _toAndroidStopSer,
                label: Text("Stop")),
          ],
        )
      ],
    ));
  }

  Future<Null> _toAndroidGetPer() async {
    bool res = await toAndroidChannel.invokeMethod("getPer");
    debugPrint("_toAndroidGetPer:$res");
  }

  Future<Null> _toAndroidStartSer() async {
    bool res = await toAndroidChannel.invokeMethod("startSer");
    debugPrint("_toAndroidStartSer:$res");
  }

  Future<Null> _toAndroidStopSer() async {
    bool res = await toAndroidChannel.invokeMethod("stopSer");
    debugPrint("_toAndroidStopSer:$res");
  }

  Future<Null> _toAndroidCheckInput() async {
    bool res = await toAndroidChannel.invokeMethod("checkInput");
    debugPrint("_toAndroidStopSer:$res");
  }
}

class PermissionRow extends StatelessWidget {
  PermissionRow(this.name, this.isOk, this.onPressed);

  final String name;
  final bool isOk;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text.rich(TextSpan(children: [
          TextSpan(
              text: name + ":",
              style: TextStyle(fontSize: 16.0, color: MyTheme.accent50)),
          TextSpan(
              text: isOk ? "已开启" : "未开启",
              style: TextStyle(
                  fontSize: 16.0, color: isOk ? Colors.green : Colors.red)),
        ])),
        TextButton(
            onPressed: onPressed,
            child: const Text(
              "去开启",
              style: TextStyle(fontWeight: FontWeight.bold),
            )),
      ],
    );
  }
}

Widget cardTitle(String text) {
  return Padding(
      padding: EdgeInsets.symmetric(vertical: 5.0),
      child: Text(
        text,
        style: TextStyle(
          fontFamily: 'WorkSans',
          fontWeight: FontWeight.bold,
          fontSize: 25,
          color: Color(0xFF00B6F0),
        ),
      ));
}

Widget myCard(Widget child) {
  return Card(
    margin: EdgeInsets.all(15.0),
    child: Padding(
      padding: EdgeInsets.symmetric(vertical: 15.0, horizontal: 30.0),
      child: child,
    ),
  );
}
