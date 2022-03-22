import 'dart:async';
import 'dart:convert';
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
  late bool _fileOk;
  final _serverId = TextEditingController(text: _emptyIdShow);
  final _serverPasswd = TextEditingController(text: "");

  List<Client> _clients = [];

  bool get isStart => _isStart;

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  bool get fileOk => _fileOk;

  TextEditingController get serverId => _serverId;

  TextEditingController get serverPasswd => _serverPasswd;

  List<Client> get clients => _clients;

  ServerModel() {
    ()async{
      await Future.delayed(Duration(seconds: 2));
      final file = FFI.getByName('option', 'enable-file-transfer');
      debugPrint("got file in option:$file");
      if (file.isEmpty) {
        _fileOk = false;
        Map<String, String> res = Map()
          ..["name"] = "enable-file-transfer"
          ..["value"] = "N";
        FFI.setByName('option', jsonEncode(res)); // 重新设置默认值
      } else {
        if (file == "Y") {
          _fileOk = true;
        } else {
          _fileOk = false;
        }
      }
    }();

  }

  toggleFile() {
    _fileOk = !_fileOk;
    Map<String, String> res = Map()
      ..["name"] = "enable-file-transfer"
      ..["value"] = _fileOk ? 'Y' : 'N';
    debugPrint("save option:$res");
    FFI.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  Future<Null> startService() async {
    _isStart = true;
    notifyListeners();
    FFI.setByName("ensure_init_event_queue");
    _interval = Timer.periodic(Duration(milliseconds: 30), (timer) {
      FFI.ffiModel.update("", (_, __) {});
    });
    await FFI.invokeMethod("init_service");
    FFI.setByName("start_service");
    getIDPasswd();
  }

  Future<Null> stopService() async {
    _isStart = false;
    release();
    FFI.serverModel.closeAll();
    await FFI.invokeMethod("stop_service");
    FFI.setByName("stop_service");
    notifyListeners();
  }

  Future<Null> initInput() async {
    await FFI.invokeMethod("init_input");
  }

  getIDPasswd() async {
    if (_serverId.text != _emptyIdShow && _serverPasswd.text != "") {
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

  release() {
    _interval?.cancel();
    _interval = null;
  }

  changeStatue(String name, bool value) {
    debugPrint("changeStatue value $value");
    switch (name) {
      case "media":
        _mediaOk = value;
        debugPrint("value $value,_isStart:$_isStart");
        if(value && !_isStart){
          startService();
        }
        break;
      case "input":
        _inputOk = value;
        //TODO change option
        break;
      default:
        return;
    }
    notifyListeners();
  }

  updateClientState() {
    var res = FFI.getByName("clients_state");
    debugPrint("getByName clients_state string:$res");
    try {
      final List clientsJson = jsonDecode(res);
      _clients = clientsJson
          .map((clientJson) => Client.fromJson(jsonDecode(res)))
          .toList();
      debugPrint("updateClientState:${_clients.toString()}");
      notifyListeners();
    } catch (e) {}
  }

  loginRequest(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt["client"]));
      final Map<String, dynamic> response = Map();
      response["id"] = client.id;
      DialogManager.show((setState, close) => CustomAlertDialog(
              title: Text("Control Request"),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(translate("Do you accept?")),
                  SizedBox(height: 20),
                  clientInfo(client),
                ],
              ),
              actions: [
                TextButton(
                    child: Text(translate("Dismiss")),
                    onPressed: () {
                      response["res"] = false;
                      FFI.setByName("login_res", jsonEncode(response));
                      close();
                    }),
                ElevatedButton(
                    child: Text(translate("Accept")),
                    onPressed: () async {
                      response["res"] = true;
                      FFI.setByName("login_res", jsonEncode(response));
                      if (!client.isFileTransfer) {
                        bool res = await FFI.invokeMethod(
                            "start_capture"); // to Android service
                        debugPrint("_toAndroidStartCapture:$res");
                      }
                      _clients.add(client);
                      notifyListeners();
                      close();
                    }),
              ]));
    } catch (e) {
      debugPrint("loginRequest failed,error:$e");
    }
  }

  void onClientLogin(Map<String, dynamic> evt) {}

  void onClientRemove(Map<String, dynamic> evt) {
    try {
      final id = int.parse(evt['id'] as String);
      Client client = _clients.singleWhere((c) => c.id == id);
      _clients.remove(client);
      notifyListeners();
    } catch (e) {
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll() {
    _clients.forEach((client) {
      FFI.setByName("close_conn", client.id.toString());
    });
    _clients = [];
  }
}

class Client {
  int id = 0; // for client connections inner count id
  bool authorized = false;
  bool isFileTransfer = false;
  String name = "";
  String peerId = ""; // for peer user's id,show at app

  Client(this.authorized, this.isFileTransfer, this.name, this.peerId);

  Client.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    authorized = json['authorized'];
    isFileTransfer = json['is_file_transfer'];
    name = json['name'];
    peerId = json['peer_id'];
  }

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> data = new Map<String, dynamic>();
    data['id'] = this.id;
    data['is_start'] = this.authorized;
    data['is_file_transfer'] = this.isFileTransfer;
    data['name'] = this.name;
    data['peer_id'] = this.peerId;
    return data;
  }
}
