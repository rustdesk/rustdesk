import 'dart:async';
import 'dart:convert';
import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';
import '../common.dart';
import '../pages/server_page.dart';
import 'model.dart';

final _emptyIdShow = translate("connecting_status");

class ServerModel with ChangeNotifier {
  Timer? _interval;
  bool _isStart = false;
  bool _mediaOk = false;
  bool _inputOk = false;
  bool _audioOk = false;
  bool _fileOk = false;
  final _serverId = TextEditingController(text: _emptyIdShow);
  final _serverPasswd = TextEditingController(text: "");

  Map<int,Client> _clients = {};

  bool get isStart => _isStart;

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  bool get audioOk => _audioOk;

  bool get fileOk => _fileOk;

  TextEditingController get serverId => _serverId;

  TextEditingController get serverPasswd => _serverPasswd;

  Map<int,Client> get clients => _clients;

  ServerModel() {
    ()async{
      await Future.delayed(Duration(seconds: 2));
      final audioOption = FFI.getByName('option', 'enable-audio');
      _audioOk = audioOption.isEmpty;   // audio true by default

      final fileOption = FFI.getByName('option', 'enable-file-transfer');
      _fileOk = fileOption.isEmpty;
      Map<String, String> res = Map()
        ..["name"] = "enable-keyboard"
        ..["value"] = 'N';
      FFI.setByName('option', jsonEncode(res)); // input false by default
      notifyListeners();
    }();
  }

  toggleAudio(){
    _audioOk = !_audioOk;
    Map<String, String> res = Map()
      ..["name"] = "enable-audio"
      ..["value"] = _audioOk ? '' : 'N';
    FFI.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  toggleFile() {
    _fileOk = !_fileOk;
    Map<String, String> res = Map()
      ..["name"] = "enable-file-transfer"
      ..["value"] = _fileOk ? '' : 'N';
    FFI.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  toggleInput(){
    if(_inputOk){
      FFI.invokeMethod("stop_input");
    }else{
      showInputWarnAlert();
    }
  }

  toggleService() async {
    if(_isStart){
      final res = await DialogManager.show<bool>((setState, close) => CustomAlertDialog(
        title: Row(children: [
          Icon(Icons.warning_amber_sharp,
              color: Colors.redAccent, size: 28),
          SizedBox(width: 10),
          Text(translate("Warning")),
        ]),
        content: Text(translate("android_stop_service_tip")),
        actions: [
          TextButton(onPressed: ()=>close(), child: Text(translate("Cancel"))),
          ElevatedButton(onPressed: ()=>close(true), child: Text(translate("OK"))),
        ],
      ));
      if(res == true){
        stopService();
      }
    }else{
      final res = await DialogManager.show<bool>((setState, close) => CustomAlertDialog(
        title: Row(children: [
          Icon(Icons.warning_amber_sharp,
              color: Colors.redAccent, size: 28),
          SizedBox(width: 10),
          Text(translate("Warning")),
        ]),
        content: Text(translate("android_service_will_start_tip")),
        actions: [
          TextButton(onPressed: ()=>close(), child: Text(translate("Cancel"))),
          ElevatedButton(onPressed: ()=>close(true), child: Text(translate("OK"))),
        ],
      ));
      if(res == true){
        startService();
      }
    }
  }

  Future<Null> startService() async {
    _isStart = true;
    notifyListeners();
    FFI.setByName("ensure_init_event_queue");
    _interval = Timer.periodic(Duration(milliseconds: 30), (timer) {
      FFI.ffiModel.update("");
    });
    await FFI.invokeMethod("init_service");
    FFI.setByName("start_service");
    getIDPasswd();
  }

  Future<Null> stopService() async {
    _isStart = false;
    _interval?.cancel();
    _interval = null;
    FFI.serverModel.closeAll();
    await FFI.invokeMethod("stop_service");
    FFI.setByName("stop_service");
    notifyListeners();
  }

  Future<Null> initInput() async {
    await FFI.invokeMethod("init_input");
  }

  Future<bool> updatePassword(String pw) async {
    final oldPasswd = _serverPasswd.text;
    FFI.setByName("update_password",pw);
    await Future.delayed(Duration(milliseconds: 500));
    await getIDPasswd(force: true);

    // check result
    if(pw == ""){
      if(_serverPasswd.text.isNotEmpty && _serverPasswd.text!= oldPasswd){
        return true;
      }else{
        return false;
      }
    }else{
      if(_serverPasswd.text == pw){
        return true;
      }else{
        return false;
      }
    }
  }

  getIDPasswd({bool force = false}) async {
    if (!force && _serverId.text != _emptyIdShow && _serverPasswd.text != "") {
      return;
    }
    var count = 0;
    const maxCount = 10;
    while (count < maxCount) {
      await Future.delayed(Duration(seconds: 1));
      final id = FFI.getByName("server_id");
      final passwd = FFI.getByName("server_password");
      if (id.isEmpty) {
        continue;
      } else {
        _serverId.text = id;
      }

      if (passwd.isEmpty) {
        continue;
      } else {
        _serverPasswd.text = passwd;
      }

      debugPrint(
          "fetch id & passwd again at $count:id:${_serverId.text},passwd:${_serverPasswd.text}");
      count++;
      if (_serverId.text != _emptyIdShow && _serverPasswd.text.isNotEmpty) {
        break;
      }
    }
    notifyListeners();
  }


  changeStatue(String name, bool value) {
    debugPrint("changeStatue value $value");
    switch (name) {
      case "media":
        _mediaOk = value;
        if(value && !_isStart){
          startService();
        }
        break;
      case "input":
        if(_inputOk!= value){
          Map<String, String> res = Map()
            ..["name"] = "enable-keyboard"
            ..["value"] = value ? '' : 'N';
          FFI.setByName('option', jsonEncode(res));
        }
        _inputOk = value;
        break;
      default:
        return;
    }
    notifyListeners();
  }

  updateClientState() {
    var res = FFI.getByName("clients_state");
    try {
      final List clientsJson = jsonDecode(res);
      for (var clientJson in clientsJson){
        final client = Client.fromJson(jsonDecode(clientJson));
        _clients[client.id] = client;
      }

      notifyListeners();
    } catch (e) {}
  }

  loginRequest(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt["client"]));
      final Map<String, dynamic> response = Map();
      response["id"] = client.id;
      DialogManager.show((setState, close) => CustomAlertDialog(
              title: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                Text(translate(client.isFileTransfer?"File Connection":"Screen Connection")),
                IconButton(onPressed: close, icon: Icon(Icons.close))
              ]),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(translate("Do you accept?")),
                  SizedBox(height: 20),
                  clientInfo(client),
                  Text(translate("android_new_connection_tip")),
                ],
              ),
              actions: [
                TextButton(
                    child: Text(translate("Dismiss")),
                    onPressed: () {
                      response["res"] = false;
                      FFI.setByName("login_res", jsonEncode(response));
                      FFI.invokeMethod("cancel_notification",client.id);
                      close();
                    }),
                ElevatedButton(
                    child: Text(translate("Accept")),
                    onPressed: () async {
                      response["res"] = true;
                      FFI.setByName("login_res", jsonEncode(response));
                      if (!client.isFileTransfer) {
                        FFI.invokeMethod("start_capture");
                      }
                      FFI.invokeMethod("cancel_notification",client.id);
                      _clients[client.id] = client;
                      notifyListeners();
                      close();
                    }),
              ],onWillPop:  ()async=>true,),barrierDismissible: true);
    } catch (e) {
      debugPrint("loginRequest failed,error:$e");
    }
  }

  void onClientAuthorized(Map<String, dynamic> evt) {
    try{
      final client = Client.fromJson(jsonDecode(evt['client']));
      // reset the login dialog, to-do,it will close any showing dialog
      DialogManager.reset();
      _clients[client.id] = client;
      notifyListeners();
    }catch(e){

    }
  }

  void onClientRemove(Map<String, dynamic> evt) {
    try {
      final id = int.parse(evt['id'] as String);
      if(_clients.containsKey(id)){
        _clients.remove(id);
      }else{
        // reset the login dialog, to-do,it will close any showing dialog
        DialogManager.reset();
        FFI.invokeMethod("cancel_notification",id);
      }
      notifyListeners();
    } catch (e) {
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll() {
    _clients.forEach((id,client) {
      FFI.setByName("close_conn", id.toString());
    });
    _clients.clear();
  }
}

class Client {
  int id = 0; // client connections inner count id
  bool authorized = false;
  bool isFileTransfer = false;
  String name = "";
  String peerId = ""; // peer user's id,show at app
  bool keyboard = false;
  bool clipboard = false;
  bool audio = false;
  late ChatUser chatUser;

  Client(this.authorized, this.isFileTransfer, this.name, this.peerId,this.keyboard,this.clipboard,this.audio);

  Client.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    authorized = json['authorized'];
    isFileTransfer = json['is_file_transfer'];
    name = json['name'];
    peerId = json['peer_id'];
    keyboard= json['keyboard'];
    clipboard= json['clipboard'];
    audio= json['audio'];
    chatUser = ChatUser(
        uid:peerId,
        name: name,
    );
  }

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> data = new Map<String, dynamic>();
    data['id'] = this.id;
    data['is_start'] = this.authorized;
    data['is_file_transfer'] = this.isFileTransfer;
    data['name'] = this.name;
    data['peer_id'] = this.peerId;
    data['keyboard'] = this.keyboard;
    data['clipboard'] = this.clipboard;
    data['audio'] = this.audio;
    return data;
  }
}

showInputWarnAlert() async {
  if (globalKey.currentContext == null) return;
  DialogManager.reset();
  await showDialog<bool>(
      context: globalKey.currentContext!,
      builder: (alertContext) {
        DialogManager.register(alertContext);
        return AlertDialog(
          title: Text(translate("How to get Android input permission?")),
          // content: Text.rich(TextSpan(style: TextStyle(), children: [
          //   // [已安装的服务] : [Installed Services]
          //   // 请在接下来的系统设置页面里，找到并进入[Installed Services]页面，将[RustDesk Input]服务开启。
          //   TextSpan(text: "请在接下来的系统设置页\n进入"),
          //   TextSpan(text: " [服务] ", style: TextStyle(color: MyTheme.accent)),
          //   TextSpan(text: "配置页面\n将"),
          //   TextSpan(
          //       text: " [RustDesk Input] ",
          //       style: TextStyle(color: MyTheme.accent)),
          //   TextSpan(text: "服务开启")
          // ])),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(translate(translate("android_input_permission_tip1"))),
              SizedBox(height: 10),
              Text(translate(translate("android_input_permission_tip2"))),
            ],
          ),
          actions: [
            TextButton(
                child: Text(translate("Cancel")),
                onPressed: () {
                  DialogManager.reset();
                }),
            ElevatedButton(
                child: Text(translate("Open System Setting")),
                onPressed: () {
                  FFI.serverModel.initInput();
                  DialogManager.reset();
                }),
          ],
        );
      });
  DialogManager.drop();
}
