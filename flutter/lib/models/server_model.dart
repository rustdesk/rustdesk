import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:wakelock/wakelock.dart';

import '../common.dart';
import '../mobile/pages/server_page.dart';
import 'model.dart';

const loginDialogTag = "LOGIN";

class ServerModel with ChangeNotifier {
  bool _isStart = false; // Android MainService status
  bool _mediaOk = false;
  bool _inputOk = false;
  bool _audioOk = false;
  bool _fileOk = false;
  int _connectStatus = 0; // Rendezvous Server status

  late String _emptyIdShow;
  late final TextEditingController _serverId;
  final _serverPasswd = TextEditingController(text: "");

  Map<int, Client> _clients = {};

  bool get isStart => _isStart;

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  bool get audioOk => _audioOk;

  bool get fileOk => _fileOk;

  int get connectStatus => _connectStatus;

  TextEditingController get serverId => _serverId;

  TextEditingController get serverPasswd => _serverPasswd;

  Map<int, Client> get clients => _clients;

  final controller = ScrollController();

  WeakReference<FFI> parent;

  ServerModel(this.parent) {
    () async {
      _emptyIdShow = translate("Generating ...", ffi: this.parent.target);
      _serverId = TextEditingController(text: this._emptyIdShow);
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
        parent.target?.setByName(
            'option',
            jsonEncode(Map()
              ..["name"] = "enable-audio"
              ..["value"] = "N"));
      } else {
        final audioOption = parent.target?.getByName('option', 'enable-audio');
        _audioOk = audioOption?.isEmpty ?? false;
      }

      // file
      if (!await PermissionManager.check("file")) {
        _fileOk = false;
        parent.target?.setByName(
            'option',
            jsonEncode(Map()
              ..["name"] = "enable-file-transfer"
              ..["value"] = "N"));
      } else {
        final fileOption =
            parent.target?.getByName('option', 'enable-file-transfer');
        _fileOk = fileOption?.isEmpty ?? false;
      }

      // input (mouse control)
      Map<String, String> res = Map()
        ..["name"] = "enable-keyboard"
        ..["value"] = 'N';
      parent.target
          ?.setByName('option', jsonEncode(res)); // input false by default
      notifyListeners();
    }();

    Timer.periodic(Duration(seconds: 1), (timer) {
      var status =
          int.tryParse(parent.target?.getByName('connect_statue') ?? "") ?? 0;
      if (status > 0) {
        status = 1;
      }
      if (status != _connectStatus) {
        _connectStatus = status;
        notifyListeners();
      }
      final res = parent.target
              ?.getByName('check_clients_length', _clients.length.toString()) ??
          "";
      if (res.isNotEmpty) {
        debugPrint("clients not match!");
        updateClientState(res);
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
    parent.target?.setByName('option', jsonEncode(res));
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
    parent.target?.setByName('option', jsonEncode(res));
    notifyListeners();
  }

  toggleInput() {
    if (_inputOk) {
      parent.target?.invokeMethod("stop_input");
    } else {
      if (parent.target != null) {
        showInputWarnAlert(parent.target!);
      }
    }
  }

  /// Toggle the screen sharing service.
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

  /// Start the screen sharing service.
  Future<Null> startService() async {
    _isStart = true;
    notifyListeners();
    // TODO
    parent.target?.ffiModel.updateEventListener("");
    await parent.target?.invokeMethod("init_service");
    parent.target?.setByName("start_service");
    getIDPasswd();
    updateClientState();
    if (!Platform.isLinux) {
      // current linux is not supported
      Wakelock.enable();
    }
  }

  /// Stop the screen sharing service.
  Future<Null> stopService() async {
    _isStart = false;
    // TODO
    parent.target?.serverModel.closeAll();
    await parent.target?.invokeMethod("stop_service");
    parent.target?.setByName("stop_service");
    notifyListeners();
    if (!Platform.isLinux) {
      // current linux is not supported
      Wakelock.disable();
    }
  }

  Future<Null> initInput() async {
    await parent.target?.invokeMethod("init_input");
  }

  Future<bool> updatePassword(String pw) async {
    final oldPasswd = _serverPasswd.text;
    parent.target?.setByName("update_password", pw);
    await Future.delayed(Duration(milliseconds: 500));
    await getIDPasswd(force: true);

    // check result
    if (pw == "") {
      if (_serverPasswd.text.isNotEmpty && _serverPasswd.text != oldPasswd) {
        return true;
      } else {
        return false;
      }
    } else {
      if (_serverPasswd.text == pw) {
        return true;
      } else {
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
      final id = parent.target?.getByName("server_id") ?? "";
      final passwd = parent.target?.getByName("server_password") ?? "";
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
        if (value && !_isStart) {
          startService();
        }
        break;
      case "input":
        if (_inputOk != value) {
          Map<String, String> res = Map()
            ..["name"] = "enable-keyboard"
            ..["value"] = value ? '' : 'N';
          parent.target?.setByName('option', jsonEncode(res));
        }
        _inputOk = value;
        break;
      default:
        return;
    }
    notifyListeners();
  }

  updateClientState([String? json]) {
    var res = json ?? parent.target?.getByName("clients_state") ?? "";
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
      parent.target?.setByName("login_res", jsonEncode(response));
      if (!client.isFileTransfer) {
        parent.target?.invokeMethod("start_capture");
      }
      parent.target?.invokeMethod("cancel_notification", client.id);
      _clients[client.id]?.authorized = true;
      notifyListeners();
    } else {
      parent.target?.setByName("login_res", jsonEncode(response));
      parent.target?.invokeMethod("cancel_notification", client.id);
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
        parent.target?.invokeMethod("cancel_notification", id);
      }
      notifyListeners();
    } catch (e) {
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll() {
    _clients.forEach((id, client) {
      parent.target?.setByName("close_conn", id.toString());
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

showInputWarnAlert(FFI ffi) {
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
                ffi.serverModel.initInput();
                close();
              }),
        ],
      ));
}
