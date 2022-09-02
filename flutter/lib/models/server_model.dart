import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:wakelock/wakelock.dart';

import '../common.dart';
import '../common/formatter/id_formatter.dart';
import '../desktop/pages/server_page.dart' as Desktop;
import '../desktop/widgets/tabbar_widget.dart';
import '../mobile/pages/server_page.dart';
import 'model.dart';

const KLoginDialogTag = "LOGIN";

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
  String _temporaryPasswordLength = "";

  late String _emptyIdShow;
  late final IDTextEditingController _serverId;
  final _serverPasswd = TextEditingController(text: "");

  final tabController = DesktopTabController(tabType: DesktopTabType.cm);

  List<Client> _clients = [];

  bool get isStart => _isStart;

  bool get mediaOk => _mediaOk;

  bool get inputOk => _inputOk;

  bool get audioOk => _audioOk;

  bool get fileOk => _fileOk;

  int get connectStatus => _connectStatus;

  String get verificationMethod {
    final index = [
      kUseTemporaryPassword,
      kUsePermanentPassword,
      kUseBothPasswords
    ].indexOf(_verificationMethod);
    if (index < 0) {
      return kUseBothPasswords;
    }
    return _verificationMethod;
  }

  set verificationMethod(String method) {
    bind.mainSetOption(key: "verification-method", value: method);
  }

  String get temporaryPasswordLength {
    final lengthIndex = ["6", "8", "10"].indexOf(_temporaryPasswordLength);
    if (lengthIndex < 0) {
      return "6";
    }
    return _temporaryPasswordLength;
  }

  set temporaryPasswordLength(String length) {
    bind.mainSetOption(key: "temporary-password-length", value: length);
  }

  TextEditingController get serverId => _serverId;

  TextEditingController get serverPasswd => _serverPasswd;

  List<Client> get clients => _clients;

  final controller = ScrollController();

  WeakReference<FFI> parent;

  ServerModel(this.parent) {
    _emptyIdShow = translate("Generating ...");
    _serverId = IDTextEditingController(text: _emptyIdShow);

    Timer.periodic(Duration(seconds: 1), (timer) async {
      var status = await bind.mainGetOnlineStatue();
      if (status > 0) {
        status = 1;
      }
      if (status != _connectStatus) {
        _connectStatus = status;
        notifyListeners();
      }
      final res = await bind.mainCheckClientsLength(length: _clients.length);
      if (res != null) {
        debugPrint("clients not match!");
        updateClientState(res);
      }

      updatePasswordModel();
    });
  }

  /// 1. check android permission
  /// 2. check config
  /// audio true by default (if permission on) (false default < Android 10)
  /// file true by default (if permission on)
  checkAndroidPermission() async {
    // audio
    if (androidVersion < 30 || !await PermissionManager.check("audio")) {
      _audioOk = false;
      bind.mainSetOption(key: "enable-audio", value: "N");
    } else {
      final audioOption = await bind.mainGetOption(key: 'enable-audio');
      _audioOk = audioOption.isEmpty;
    }

    // file
    if (!await PermissionManager.check("file")) {
      _fileOk = false;
      bind.mainSetOption(key: "enable-file-transfer", value: "N");
    } else {
      final fileOption = await bind.mainGetOption(key: 'enable-file-transfer');
      _fileOk = fileOption.isEmpty;
    }

    // input (mouse control) false by default
    bind.mainSetOption(key: "enable-keyboard", value: "N");
    notifyListeners();
  }

  updatePasswordModel() async {
    var update = false;
    final temporaryPassword = await bind.mainGetTemporaryPassword();
    final verificationMethod =
        await bind.mainGetOption(key: "verification-method");
    final temporaryPasswordLength =
        await bind.mainGetOption(key: "temporary-password-length");
    final oldPwdText = _serverPasswd.text;
    if (_serverPasswd.text != temporaryPassword) {
      _serverPasswd.text = temporaryPassword;
    }
    if (verificationMethod == kUsePermanentPassword) {
      _serverPasswd.text = '-';
    }
    if (oldPwdText != _serverPasswd.text) {
      update = true;
    }
    if (_verificationMethod != verificationMethod) {
      _verificationMethod = verificationMethod;
      update = true;
    }
    if (_temporaryPasswordLength != temporaryPasswordLength) {
      _temporaryPasswordLength = temporaryPasswordLength;
      update = true;
    }
    if (update) {
      notifyListeners();
    }
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
    bind.mainSetOption(key: "enable-audio", value: _audioOk ? '' : 'N');
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
    bind.mainSetOption(key: "enable-file-transfer", value: _fileOk ? '' : 'N');
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
      final res = await parent.target?.dialogManager
          .show<bool>((setState, close) => CustomAlertDialog(
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
      final res = await parent.target?.dialogManager
          .show<bool>((setState, close) => CustomAlertDialog(
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
    await bind.mainStartService();
    _fetchID();
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
    closeAll();
    await parent.target?.invokeMethod("stop_service");
    await bind.mainStopService();
    notifyListeners();
    if (!Platform.isLinux) {
      // current linux is not supported
      Wakelock.disable();
    }
  }

  Future<Null> initInput() async {
    await parent.target?.invokeMethod("init_input");
  }

  Future<bool> setPermanentPassword(String newPW) async {
    await bind.mainSetPermanentPassword(password: newPW);
    await Future.delayed(Duration(milliseconds: 500));
    final pw = await bind.mainGetPermanentPassword();
    if (newPW == pw) {
      return true;
    } else {
      return false;
    }
  }

  _fetchID() async {
    final old = _serverId.id;
    var count = 0;
    const maxCount = 10;
    while (count < maxCount) {
      await Future.delayed(Duration(seconds: 1));
      final id = await bind.mainGetMyId();
      if (id.isEmpty) {
        continue;
      } else {
        _serverId.id = id;
      }

      debugPrint("fetch id again at $count:id:${_serverId.id}");
      count++;
      if (_serverId.id != old) {
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
          bind.mainSetOption(key: "enable-keyboard", value: value ? '' : 'N');
        }
        _inputOk = value;
        break;
      default:
        return;
    }
    notifyListeners();
  }

  // force
  updateClientState([String? json]) async {
    var res = await bind.mainGetClientsState();
    try {
      final List clientsJson = jsonDecode(res);
      _clients.clear();
      tabController.state.value.tabs.clear();
      for (var clientJson in clientsJson) {
        final client = Client.fromJson(clientJson);
        _clients.add(client);
        tabController.add(
            TabInfo(
                key: client.id.toString(),
                label: client.name,
                closable: false,
                page: Desktop.buildConnectionCard(client)),
            authorized: client.authorized);
      }
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to updateClientState:$e");
    }
  }

  void loginRequest(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt["client"]));
      if (_clients.any((c) => c.id == client.id)) {
        return;
      }
      _clients.add(client);
      tabController.add(TabInfo(
          key: client.id.toString(),
          label: client.name,
          closable: false,
          page: Desktop.buildConnectionCard(client)));
      scrollToBottom();
      notifyListeners();
      if (isAndroid) showLoginDialog(client);
    } catch (e) {
      debugPrint("Failed to call loginRequest,error:$e");
    }
  }

  void showLoginDialog(Client client) {
    parent.target?.dialogManager.show(
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
    if (isDesktop) return;
    Future.delayed(Duration(milliseconds: 200), () {
      controller.animateTo(controller.position.maxScrollExtent,
          duration: Duration(milliseconds: 200),
          curve: Curves.fastLinearToSlowEaseIn);
    });
  }

  void sendLoginResponse(Client client, bool res) async {
    if (res) {
      bind.cmLoginRes(connId: client.id, res: res);
      if (!client.isFileTransfer) {
        parent.target?.invokeMethod("start_capture");
      }
      parent.target?.invokeMethod("cancel_notification", client.id);
      client.authorized = true;
      notifyListeners();
    } else {
      bind.cmLoginRes(connId: client.id, res: res);
      parent.target?.invokeMethod("cancel_notification", client.id);
      final index = _clients.indexOf(client);
      tabController.remove(index);
      _clients.remove(client);
    }
  }

  void onClientAuthorized(Map<String, dynamic> evt) {
    try {
      final client = Client.fromJson(jsonDecode(evt['client']));
      parent.target?.dialogManager.dismissByTag(getLoginDialogTag(client.id));
      final index = _clients.indexWhere((c) => c.id == client.id);
      if (index < 0) {
        _clients.add(client);
      } else {
        _clients[index].authorized = true;
      }
      tabController.add(
          TabInfo(
              key: client.id.toString(),
              label: client.name,
              closable: false,
              page: Desktop.buildConnectionCard(client)),
          authorized: true);
      scrollToBottom();
      notifyListeners();
    } catch (e) {
      debugPrint("onClientAuthorized:$e");
    }
  }

  void onClientRemove(Map<String, dynamic> evt) {
    try {
      final id = int.parse(evt['id'] as String);
      if (_clients.any((c) => c.id == id)) {
        final index = _clients.indexWhere((client) => client.id == id);
        if (index >= 0) {
          _clients.removeAt(index);
          tabController.remove(index);
        }
        parent.target?.dialogManager.dismissByTag(getLoginDialogTag(id));
        parent.target?.invokeMethod("cancel_notification", id);
      }
      notifyListeners();
    } catch (e) {
      debugPrint("onClientRemove failed,error:$e");
    }
  }

  closeAll() {
    _clients.forEach((client) {
      bind.cmCloseConnection(connId: client.id);
    });
    _clients.clear();
    tabController.state.value.tabs.clear();
  }

  void jumpTo(int id) {
    final index = _clients.indexWhere((client) => client.id == id);
    tabController.jumpTo(index);
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
  bool file = false;
  bool restart = false;

  Client(this.id, this.authorized, this.isFileTransfer, this.name, this.peerId,
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
    file = json['file'];
    restart = json['restart'];
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
  return KLoginDialogTag + id.toString();
}

showInputWarnAlert(FFI ffi) {
  ffi.dialogManager.show((setState, close) => CustomAlertDialog(
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
