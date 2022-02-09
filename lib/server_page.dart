import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/model.dart';
import 'package:provider/provider.dart';

import 'common.dart';
import 'main.dart';

class ServerPage extends StatelessWidget {
  static final serverModel = ServerModel();

  @override
  Widget build(BuildContext context) {
    checkService();
    return ChangeNotifierProvider.value(
        value: serverModel,
        child: Scaffold(
            backgroundColor: MyTheme.grayBg,
            appBar: AppBar(
              centerTitle: true,
              title: const Text("Share My Screen"),
              actions: [
                PopupMenuButton<String>(
                    itemBuilder: (context) {
                      return [
                        PopupMenuItem(
                          child: Text("修改服务ID"),
                          value: "changeID",
                          enabled: false,
                        ),
                        PopupMenuItem(
                          child: Text("修改服务密码"),
                          value: "changeID",
                          enabled: false,
                        )
                      ];
                    },
                    onSelected: (value) =>
                        debugPrint("PopupMenuItem onSelected:$value"))
              ],
            ),
            body: SingleChildScrollView(
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  children: [
                    ServerInfo(),
                    PermissionChecker(),
                    ConnectionManager(),
                    SizedBox.fromSize(size: Size(0, 15.0)), // Bottom padding
                  ],
                ),
              ),
            )));
  }
}

void checkService() {
  // 检测当前服务状态，若已存在服务则异步更新数据回来
  toAndroidChannel.invokeMethod("check_service"); // jvm
  ServerPage.serverModel.updateClientState();
}

class ServerInfo extends StatefulWidget {
  @override
  _ServerInfoState createState() => _ServerInfoState();
}

class _ServerInfoState extends State<ServerInfo> {
  var _passwdShow = false;

  // TODO set ID / PASSWORD
  var _serverId = TextEditingController(text: "");
  var _serverPasswd = TextEditingController(text: "");
  static const _emptyIdShow = "正在获取ID...";


  @override
  void initState() {
    super.initState();
    var id = FFI.getByName("server_id");
    _serverId.text = id==""?_emptyIdShow:id;
    _serverPasswd.text = FFI.getByName("server_password");
    if(_serverId.text == _emptyIdShow || _serverPasswd.text == ""){
      fetchConfigAgain();
    }
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
          controller: _serverId,
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
          obscureText: !_passwdShow,
          style: TextStyle(
              fontSize: 25.0,
              fontWeight: FontWeight.bold,
              color: MyTheme.accent),
          controller: _serverPasswd,
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
  fetchConfigAgain()async{
    FFI.setByName("start_service");
    var count = 0;
    const maxCount = 10;
    while(count<maxCount){
      if(_serverId.text!=_emptyIdShow && _serverPasswd.text!=""){
        break;
      }
      await Future.delayed(Duration(seconds: 2));
      var id = FFI.getByName("server_id");
      _serverId.text = id==""?_emptyIdShow:id;
      _serverPasswd.text = FFI.getByName("server_password");
      debugPrint("fetch id & passwd again at $count:id:${_serverId.text},passwd:${_serverPasswd.text}");
      count++;
    }
    FFI.setByName("stop_service");
  }
}

class PermissionChecker extends StatefulWidget {
  @override
  _PermissionCheckerState createState() => _PermissionCheckerState();
}

class _PermissionCheckerState extends State<PermissionChecker> {
  @override
  void initState() {
    super.initState();
    nowCtx = context;
  }

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);

    return myCard(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        cardTitle("权限列表"),
        PermissionRow("媒体权限", serverModel.mediaOk, _toAndroidInitService),
        const Divider(height: 0),
        PermissionRow("输入权限", serverModel.inputOk, _toAndroidInitInput),
        const Divider(),
        serverModel.mediaOk
            ? ElevatedButton.icon(
                icon: Icon(Icons.stop),
                onPressed: _toAndroidStopService,
                label: Text("Stop"))
            : ElevatedButton.icon(
                icon: Icon(Icons.play_arrow),
                onPressed: _toAndroidInitService,
                label: Text("Start")),
      ],
    ));
  }
}

BuildContext loginReqAlertCtx;

void showLoginReqAlert(BuildContext context, String peerID, String name)async {
  debugPrint("got try_start_without_auth");
  await showDialog(
      context: context,
      builder: (alertContext) {
        loginReqAlertCtx = alertContext;
        return AlertDialog(
          title: Text("收到连接请求"),
          content: Text("是否同意来自$name:$peerID的控制？"),
          actions: [
            TextButton(
                child: Text("接受"),
                onPressed: () {
                  FFI.setByName("login_res", "true");
                  if (!ServerPage.serverModel.isFileTransfer) {
                    _toAndroidStartCapture();
                  }
                  ServerPage.serverModel.setPeer(true);
                  Navigator.of(alertContext).pop();
                }),
            TextButton(
                child: Text("不接受"),
                onPressed: () {
                  FFI.setByName("login_res", "false");
                  Navigator.of(alertContext).pop();
                })
          ],
        );
      });
  debugPrint("alert done");
  loginReqAlertCtx = null;
}

clearLoginReqAlert(){
  if (loginReqAlertCtx!=null){
    Navigator.of(loginReqAlertCtx).pop();
    ServerPage.serverModel.updateClientState();
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
              text: name + ":  ",
              style: TextStyle(fontSize: 16.0, color: MyTheme.accent50)),
          TextSpan(
              text: isOk ? "已开启" : "未开启",
              style: TextStyle(
                  fontSize: 16.0, color: isOk ? Colors.green : Colors.grey)),
        ])),
        TextButton(
            onPressed: isOk ? null : onPressed,
            child: const Text(
              "去开启",
              style: TextStyle(fontWeight: FontWeight.bold),
            )),
      ],
    );
  }
}

class ConnectionManager extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    var info =
        "${serverModel.peerName != "" ? serverModel.peerName : "NA"}-${serverModel.peerID != "" ? serverModel.peerID : "NA"}";
    return serverModel.isPeerStart
        ? myCard(Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              cardTitle("当前连接"),
              Padding(
                padding: EdgeInsets.symmetric(vertical: 5.0),
                child: Text(info, style: TextStyle(color: Colors.grey)),
              ),
              ElevatedButton.icon(
                  style: ButtonStyle(
                      backgroundColor: MaterialStateProperty.all(Colors.red)),
                  icon: Icon(Icons.close),
                  onPressed: () {
                    FFI.setByName("close_conn");
                    // _toAndroidStopCapture();
                    serverModel.setPeer(false);
                  },
                  label: Text("断开连接"))
            ],
          ))
        : SizedBox.shrink();
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
          fontSize: 22,
          color: MyTheme.accent80,
        ),
      ));
}

Widget myCard(Widget child) {
  return Container(
      width: double.maxFinite,
      child: Card(
        margin: EdgeInsets.fromLTRB(15.0, 15.0, 15.0, 0),
        child: Padding(
          padding: EdgeInsets.symmetric(vertical: 15.0, horizontal: 30.0),
          child: child,
        ),
      ));
}

Future<Null> _toAndroidInitService() async {
  bool res = await toAndroidChannel.invokeMethod("init_service");
  FFI.setByName("start_service");
  debugPrint("_toAndroidInitService:$res");
}

Future<Null> _toAndroidStartCapture() async {
  bool res = await toAndroidChannel.invokeMethod("start_capture");
  debugPrint("_toAndroidStartCapture:$res");
}

// Future<Null> _toAndroidStopCapture() async {
//   bool res = await toAndroidChannel.invokeMethod("stop_capture");
//   debugPrint("_toAndroidStopCapture:$res");
// }

Future<Null> _toAndroidStopService() async {
  FFI.setByName("close_conn");
  ServerPage.serverModel.setPeer(false);

  bool res = await toAndroidChannel.invokeMethod("stop_service");
  FFI.setByName("stop_service");
  debugPrint("_toAndroidStopSer:$res");
}

Future<Null> _toAndroidInitInput() async {
  bool res = await toAndroidChannel.invokeMethod("init_input");
  debugPrint("_toAndroidInitInput:$res");
}
