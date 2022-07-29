import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:wakelock/wakelock.dart';
import '../common.dart';
import '../pages/server_page.dart';
import 'model.dart';

const loginDialogTag = "LOGIN";
final _emptyIdShow = translate("Generating ...");

const kUseTemporaryPassword = "use-temporary-password";
const kUsePermanentPassword = "use-permanent-password";
const kUseBothPasswords = "use-both-passwords";

class ServerModel with ChangeNotifier {
  bool _isStart = false; // Android MainService status
  bool _mediaOk = false;
  bool _inputOk = false;
  bool _audioOk = false;
  bool _fileOk = false;
  int _connectStatus = 0; // Rendezvous Server status
  String _verificationMethod = "";

  final _serverId = TextEditingController(text: _emptyIdShow);
  final _serverPasswd = TextEditingController(text: "");

  Map<int, Client> _clients = {};

  bool get isStart => _isStart;

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  bool get audioOk => _audioOk;

  bool get fileOk => _fileOk;

  int get connectStatus => _connectStatus;

  String get verificationMethod => _verificationMethod;

  TextEditingController get serverId => _serverId;

  TextEditingController get serverPasswd => _serverPasswd;

  Map<int, Client> get clients => _clients;

  final controller = ScrollController();

  ServerModel() {
    () async {
      /**
       * 1. check android permission
       * 2. check config
       * audio true by default (if permission on) (false default < Android 10)
       * file true by default (if permission on)
       */
      await Future.delayed(Duration(seconds: 1));

      // audio
      if (androidVersion < 30 || !await PermissionManager.check("audio")) {
        _audioOk = false;
        FFI.setByName(
            'option',
            jsonEncode(Map()
              ..["name"] = "enable-audio"
              ..["value"] = "N"));
      } else {
        final audioOption = FFI.getByName('option', 'enable-audio');
        _audioOk = audioOption.isEmpty;
      }

      // file
      if (!await PermissionManager.check("file")) {
        _fileOk = false;
        FFI.setByName(
            'option',
            jsonEncode(Map()
              ..["name"] = "enable-file-transfer"
              ..["value"] = "N"));
      } else {
        final fileOption = FFI.getByName('option', 'enable-file-transfer');
        _fileOk = fileOption.isEmpty;
      }

      notifyListeners();
    }();

    Timer.periodic(Duration(seconds: 1), (timer) {
      var update = false;
      var status = int.tryParse(FFI.getByName('connect_statue')) ?? 0;
      if (status > 0) {
        status = 1;
      }
      if (status != _connectStatus) {
        _connectStatus = status;
        update = true;
      }
      final res =
          FFI.getByName('check_clients_length', _clients.length.toString());
      if (res.isNotEmpty) {
        debugPrint("clients not match!");
        updateClientState(res);
      }

      final temporaryPassword = FFI.getByName("temporary_password");
      final verificationMethod = FFI.getByName("option", "verification-method");
      if (_serverPasswd.text != temporaryPassword) {
        _serverPasswd.text = temporaryPassword;
        update = true;
      }

      if (_verificationMethod != verificationMethod) {
        debugPrint("_verificationMethod changed: $verificationMethod");
        _verificationMethod = verificationMethod;
        update = true;
      }

      if (update) {
        notifyListeners();
      }
    });
  }

  toggleAudio() async {
    if (!_audioOk && !await PermissionManager.check("audio")) {
      final res = await PermissionManager.request("audio");
      if (!res) {
        // TODO handle fail
        return;
      }
    }

    _audioOk = !_audioOk;
    Map<String, String> res = Map()
      ..["name"] = "enable-audio"
      ..["value"] = _audioOk ? '' : 'N';
    FFI.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  toggleFile() async {
    if (!_fileOk && !await PermissionManager.check("file")) {
      final res = await PermissionManager.request("file");
      if (!res) {
        // TODO handle fail
        return;
      }
    }

    _fileOk = !_fileOk;
    Map<String, String> res = Map()
      ..["name"] = "enable-file-transfer"
      ..["value"] = _fileOk ? '' : 'N';
    FFI.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  toggleInput() {
    if (_inputOk) {
      FFI.invokeMethod("stop_input");
    } else {
      showInputWarnAlert();
    }
  }

  toggleService() async {
    if (_isStart) {
      final res =
          await DialogManager.show<bool>((setState, close) => CustomAlertDialog(
                title: Row(children: [
                  Icon(Icons.warning_amber_sharp,
                      color: Colors.redAccent, size: 28),
                  SizedBox(width: 10),
                  Text(translate("Warning")),
                ]),
                content: Text(translate("android_stop_service_tip")),
                actions: [
                  TextButton(
                      onPressed: () => close(),
                      child: Text(translate("Cancel"))),
                  ElevatedButton(
                      onPressed: () => close(true),
                      child: Text(translate("OK"))),
                ],
              ));
      if (res == true) {
        stopService();
      }
    } else {
      final res =
          await DialogManager.show<bool>((setState, close) => CustomAlertDialog(
                title: Row(children: [
                  Icon(Icons.warning_amber_sharp,
                      color: Colors.redAccent, size: 28),
                  SizedBox(width: 10),
                  Text(translate("Warning")),
                ]),
                content: Text(translate("android_service_will_start_tip")),
                actions: [
                  TextButton(
                      onPressed: () => close(),
                      child: Text(translate("Cancel"))),
                  ElevatedButton(
                      onPressed: () => close(true),
                      child: Text(translate("OK"))),
                ],
              ));
      if (res == true) {
        startService();
      }
    }
  }

  Future<Null> startService() async {
    _isStart = true;
    notifyListeners();
    FFI.ffiModel.updateEventListener("");
    await FFI.invokeMethod("init_service");
    FFI.setByName("start_service");
    _fetchID();
    updateClientState();
    Wakelock.enable();
  }

  Future<Null> stopService() async {
    _isStart = false;
    FFI.serverModel.closeAll();
    await FFI.invokeMethod("stop_service");
    FFI.setByName("stop_service");
    notifyListeners();
    Wakelock.disable();
  }

  Future<Null> initInput() async {
    await FFI.invokeMethod("init_input");
  }

  Future<bool> setPermanentPassword(String newPW) async {
    FFI.setByName("permanent_password", newPW);
    await Future.delayed(Duration(milliseconds: 500));
    final pw = FFI.getByName("permanent_password", newPW);
    if (newPW == pw) {
      return true;
    } else {
      return false;
    }
  }

  _fetchID() async {
    final old = _serverId.text;
    var count = 0;
    const maxCount = 10;
    while (count < maxCount) {
      await Future.delayed(Duration(seconds: 1));
      final id = FFI.getByName("server_id");
      if (id.isEmpty) {
        continue;
      } else {
        _serverId.text = id;
      }

      debugPrint("fetch id again at $count:id:${_serverId.text}");
      count++;
      if (_serverId.text != old) {
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
        if (value && !_isStart) {
          startService();
        }
        break;
      case "input":
        if (_inputOk != value) {
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

  updateClientState([String? json]) {
    var res = json ?? FFI.getByName("clients_state");
    try {
      final List clientsJson = jsonDecode(res);
      for (var clientJson in clientsJson) {
        final client = Client.fromJson(clientJson);
        _clients[client.id] = client;
      }
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to updateClientState:$e");
    }
  }

  void loginRequest(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt["client"]));
      if (_clients.containsKey(client.id)) {
        return;
      }
      _clients[client.id] = client;
      scrollToBottom();
      notifyListeners();
      showLoginDialog(client);
    } catch (e) {
      debugPrint("Failed to call loginRequest,error:$e");
    }
  }

  void showLoginDialog(Client client) {
    DialogManager.show(
        (setState, close) => CustomAlertDialog(
              title: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(translate(client.isFileTransfer
                        ? "File Connection"
                        : "Screen Connection")),
                    IconButton(
                        onPressed: () {
                          close();
                        },
                        icon: Icon(Icons.close))
                  ]),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(translate("Do you accept?")),
                  clientInfo(client),
                  Text(
                    translate("android_new_connection_tip"),
                    style: TextStyle(color: Colors.black54),
                  ),
                ],
              ),
              actions: [
                TextButton(
                    child: Text(translate("Dismiss")),
                    onPressed: () {
                      sendLoginResponse(client, false);
                      close();
                    }),
                ElevatedButton(
                    child: Text(translate("Accept")),
                    onPressed: () {
                      sendLoginResponse(client, true);
                      close();
                    }),
              ],
            ),
        tag: getLoginDialogTag(client.id));
  }

  scrollToBottom() {
    Future.delayed(Duration(milliseconds: 200), () {
      controller.animateTo(controller.position.maxScrollExtent,
          duration: Duration(milliseconds: 200),
          curve: Curves.fastLinearToSlowEaseIn);
    });
  }

  void sendLoginResponse(Client client, bool res) {
    final Map<String, dynamic> response = Map();
    response["id"] = client.id;
    response["res"] = res;
    if (res) {
      FFI.setByName("login_res", jsonEncode(response));
      if (!client.isFileTransfer) {
        FFI.invokeMethod("start_capture");
      }
      FFI.invokeMethod("cancel_notification", client.id);
      _clients[client.id]?.authorized = true;
      notifyListeners();
    } else {
      FFI.setByName("login_res", jsonEncode(response));
      FFI.invokeMethod("cancel_notification", client.id);
      _clients.remove(client.id);
    }
  }

  void onClientAuthorized(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt['client']));
      DialogManager.dismissByTag(getLoginDialogTag(client.id));
      _clients[client.id] = client;
      scrollToBottom();
      notifyListeners();
    } catch (e) {}
  }

  void onClientRemove(Map<String, dynamic> evt) {
    try {
      final id = int.parse(evt['id'] as String);
      if (_clients.containsKey(id)) {
        _clients.remove(id);
        DialogManager.dismissByTag(getLoginDialogTag(id));
        FFI.invokeMethod("cancel_notification", id);
      }
      notifyListeners();
    } catch (e) {
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll() {
    _clients.forEach((id, client) {
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

  Client(this.authorized, this.isFileTransfer, this.name, this.peerId,
      this.keyboard, this.clipboard, this.audio);

  Client.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    authorized = json['authorized'];
    isFileTransfer = json['is_file_transfer'];
    name = json['name'];
    peerId = json['peer_id'];
    keyboard = json['keyboard'];
    clipboard = json['clipboard'];
    audio = json['audio'];
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

String getLoginDialogTag(int id) {
  return loginDialogTag + id.toString();
}

showInputWarnAlert() {
  DialogManager.show((setState, close) => CustomAlertDialog(
        title: Text(translate("How to get Android input permission?")),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(translate("android_input_permission_tip1")),
            SizedBox(height: 10),
            Text(translate("android_input_permission_tip2")),
          ],
        ),
        actions: [
          TextButton(child: Text(translate("Cancel")), onPressed: close),
          ElevatedButton(
              child: Text(translate("Open System Setting")),
              onPressed: () {
                FFI.serverModel.initInput();
                close();
              }),
        ],
      ));
}
