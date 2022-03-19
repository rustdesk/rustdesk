import 'dart:convert';
import 'package:flutter/material.dart';
import '../common.dart';
import '../pages/server_page.dart';
import 'model.dart';

class ServerModel with ChangeNotifier {
  bool _mediaOk = false;
  bool _inputOk = false;
  List<Client> _clients = [];

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  List<Client> get clients => _clients;

  ServerModel();

  changeStatue(String name, bool value) {
    switch (name) {
      case "media":
        _mediaOk = value;
        break;
      case "input":
        _inputOk = value;
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
      _clients = clientsJson.map((clientJson) => Client.fromJson(jsonDecode(res))).toList();
      debugPrint("updateClientState:${_clients.toString()}");
      notifyListeners();
    } catch (e) {}
  }

  loginRequest(Map<String, dynamic> evt){
    try{
      final client = Client.fromJson(jsonDecode(evt["client"]));
      final Map<String,dynamic> response = Map();
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
                  if (!client.isFileTransfer)  {
                    bool res = await FFI.invokeMethod("start_capture"); // to Android service
                    debugPrint("_toAndroidStartCapture:$res");
                  }
                  _clients.add(client);
                  notifyListeners();
                  close();
                }),

          ]));
    }catch (e){
      debugPrint("loginRequest failed,error:$e");
    }
  }

  void onClientLogin(Map<String, dynamic> evt){

  }

  void onClientRemove(Map<String, dynamic> evt) {
    try{
      final id = int.parse(evt['id'] as String);
      Client client = _clients.singleWhere((c) => c.id == id);
      _clients.remove(client);
      notifyListeners();
    }catch(e){
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll(){
    _clients.forEach((client) {
      FFI.setByName("close_conn",client.id.toString());
    });
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

