import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/dialog.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../common.dart';
import '../../common/widgets/dialog.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';
import 'home_page.dart';

class ServerPage extends StatefulWidget implements PageShape {
  @override
  final title = translate("Share Screen");

  @override
  final icon = const Icon(Icons.mobile_screen_share);

  @override
  final appBarActions = [
    PopupMenuButton<String>(
        icon: const Icon(Icons.more_vert),
        itemBuilder: (context) {
          return [
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 16.0),
              value: "changeID",
              child: Text(translate("Change ID")),
            ),
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 16.0),
              value: "setPermanentPassword",
              enabled:
                  gFFI.serverModel.verificationMethod != kUseTemporaryPassword,
              child: Text(translate("Set permanent password")),
            ),
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 16.0),
              value: "setTemporaryPasswordLength",
              enabled:
                  gFFI.serverModel.verificationMethod != kUsePermanentPassword,
              child: Text(translate("One-time password length")),
            ),
            const PopupMenuDivider(),
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 0.0),
              value: kUseTemporaryPassword,
              child: ListTile(
                  title: Text(translate("Use one-time password")),
                  trailing: Icon(
                    Icons.check,
                    color: gFFI.serverModel.verificationMethod ==
                            kUseTemporaryPassword
                        ? null
                        : Colors.transparent,
                  )),
            ),
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 0.0),
              value: kUsePermanentPassword,
              child: ListTile(
                  title: Text(translate("Use permanent password")),
                  trailing: Icon(
                    Icons.check,
                    color: gFFI.serverModel.verificationMethod ==
                            kUsePermanentPassword
                        ? null
                        : Colors.transparent,
                  )),
            ),
            PopupMenuItem(
              padding: const EdgeInsets.symmetric(horizontal: 0.0),
              value: kUseBothPasswords,
              child: ListTile(
                  title: Text(translate("Use both passwords")),
                  trailing: Icon(
                    Icons.check,
                    color: gFFI.serverModel.verificationMethod !=
                                kUseTemporaryPassword &&
                            gFFI.serverModel.verificationMethod !=
                                kUsePermanentPassword
                        ? null
                        : Colors.transparent,
                  )),
            ),
          ];
        },
        onSelected: (value) {
          if (value == "changeID") {
            changeIdDialog();
          } else if (value == "setPermanentPassword") {
            setPermanentPasswordDialog(gFFI.dialogManager);
          } else if (value == "setTemporaryPasswordLength") {
            setTemporaryPasswordLengthDialog(gFFI.dialogManager);
          } else if (value == kUsePermanentPassword ||
              value == kUseTemporaryPassword ||
              value == kUseBothPasswords) {
            bind.mainSetOption(key: "verification-method", value: value);
            gFFI.serverModel.updatePasswordModel();
          }
        })
  ];

  ServerPage({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _ServerPageState();
}

class _ServerPageState extends State<ServerPage> {
  Timer? _updateTimer;

  @override
  void initState() {
    super.initState();
    _updateTimer = periodic_immediate(const Duration(seconds: 3), () async {
      await gFFI.serverModel.fetchID();
    });
    gFFI.serverModel.checkAndroidPermission();
  }

  @override
  void dispose() {
    _updateTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    checkService();
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(
            builder: (context, serverModel, child) => SingleChildScrollView(
                  controller: gFFI.serverModel.controller,
                  child: Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.start,
                      children: [
                        ServerInfo(),
                        const ConnectionManager(),
                        const PermissionChecker(),
                        SizedBox.fromSize(size: const Size(0, 15.0)),
                      ],
                    ),
                  ),
                )));
  }
}

void checkService() async {
  gFFI.invokeMethod("check_service");
  // for Android 10/11, request MANAGE_EXTERNAL_STORAGE permission from system setting page
  if (AndroidPermissionManager.isWaitingFile() && !gFFI.serverModel.fileOk) {
    AndroidPermissionManager.complete(kManageExternalStorage,
        await AndroidPermissionManager.check(kManageExternalStorage));
    debugPrint("file permission finished");
  }
}

class ServerInfo extends StatelessWidget {
  final model = gFFI.serverModel;
  final emptyController = TextEditingController(text: "-");

  ServerInfo({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final isPermanent = model.verificationMethod == kUsePermanentPassword;
    final serverModel = Provider.of<ServerModel>(context);

    // @todo Theming
    Widget Notification() {
      const Color colorPositive = Colors.greenAccent;
      const Color colorNegative = Colors.redAccent;
      const double paddingRight = 15;

      if (serverModel.connectStatus == -1) {
        return Row(children: [
          const Icon(Icons.warning_amber_sharp, color: colorNegative, size: 24)
              .marginOnly(right: paddingRight),
          Expanded(child: Text(translate('not_ready_status')))
        ]);
      } else if (serverModel.connectStatus == 0) {
        return Row(children: [
          SizedBox(width: 20, height: 20, child: CircularProgressIndicator())
              .marginOnly(left: 4, right: paddingRight),
          Expanded(child: Text(translate('connecting_status')))
        ]);
      } else {
        return Row(children: [
          const Icon(Icons.check, color: colorPositive, size: 24)
              .marginOnly(right: paddingRight),
          Expanded(child: Text(translate('Ready')))
        ]);
      }
    }

    return model.isStart
        ? PaddingCard(
            child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(
                readOnly: true,
                style: const TextStyle(
                    fontSize: 25.0, fontWeight: FontWeight.bold),
                controller: model.serverId,
                decoration: InputDecoration(
                  icon: const Icon(Icons.perm_identity),
                  labelText: translate("ID"),
                  labelStyle: const TextStyle(fontWeight: FontWeight.bold),
                ),
                onSaved: (String? value) {},
              ),
              TextFormField(
                readOnly: true,
                style: const TextStyle(
                    fontSize: 25.0, fontWeight: FontWeight.bold),
                controller: isPermanent ? emptyController : model.serverPasswd,
                decoration: InputDecoration(
                    icon: const Icon(Icons.lock),
                    labelText: translate("Password"),
                    labelStyle: const TextStyle(
                      fontWeight: FontWeight.bold,
                    ),
                    suffix: isPermanent
                        ? null
                        : IconButton(
                            icon: const Icon(Icons.refresh),
                            onPressed: () =>
                                bind.mainUpdateTemporaryPassword())),
                onSaved: (String? value) {},
              ),
              Notification().marginOnly(top: 20)
            ],
          ))
        : PaddingCard(
            child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Icon(Icons.warning_amber_sharp,
                      color: Colors.redAccent, size: 24),
                  const SizedBox(width: 10),
                  Expanded(
                      child: Text(
                    translate("Service is not running"),
                    style: const TextStyle(
                      fontFamily: 'WorkSans',
                      fontWeight: FontWeight.bold,
                      fontSize: 18,
                    ),
                  ))
                ],
              ).marginOnly(bottom: 8),
              Text(translate("android_start_service_tip"),
                      style: const TextStyle(
                          fontSize: 12, color: MyTheme.darkGray))
                  .marginOnly(bottom: 8),
              ElevatedButton.icon(
                  icon: const Icon(Icons.play_arrow),
                  onPressed: serverModel.toggleService,
                  label: Text(translate("Start Service")))
            ],
          ));
  }
}

class PermissionChecker extends StatefulWidget {
  const PermissionChecker({Key? key}) : super(key: key);

  @override
  State<PermissionChecker> createState() => _PermissionCheckerState();
}

class _PermissionCheckerState extends State<PermissionChecker> {
  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    final hasAudioPermission = androidVersion >= 30;
    return PaddingCard(
        title: translate("Permissions"),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          serverModel.mediaOk
              ? ElevatedButton.icon(
                      style: ButtonStyle(
                          backgroundColor:
                              MaterialStateProperty.all(Colors.red)),
                      icon: const Icon(Icons.stop),
                      onPressed: serverModel.toggleService,
                      label: Text(translate("Stop service")))
                  .marginOnly(bottom: 8)
              : SizedBox.shrink(),
          PermissionRow(translate("Screen Capture"), serverModel.mediaOk,
              serverModel.toggleService),
          PermissionRow(translate("Input Control"), serverModel.inputOk,
              serverModel.toggleInput),
          PermissionRow(translate("Transfer File"), serverModel.fileOk,
              serverModel.toggleFile),
          hasAudioPermission
              ? PermissionRow(translate("Audio Capture"), serverModel.audioOk,
                  serverModel.toggleAudio)
              : Text(
                  "* ${translate("android_version_audio_tip")}",
                  style: const TextStyle(color: MyTheme.darkGray),
                )
        ]));
  }
}

class PermissionRow extends StatelessWidget {
  const PermissionRow(this.name, this.isOk, this.onPressed, {Key? key})
      : super(key: key);

  final String name;
  final bool isOk;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
            flex: 5,
            child: FittedBox(
                fit: BoxFit.scaleDown,
                alignment: Alignment.centerLeft,
                child:
                    Text(name, style: Theme.of(context).textTheme.labelLarge))),
        Expanded(
          flex: 2,
          child: FittedBox(
              fit: BoxFit.scaleDown,
              child: Text(isOk ? translate("ON") : translate("OFF"),
                  style: TextStyle(
                      fontSize: 16.0,
                      color: isOk ? Colors.green : Colors.grey))),
        ),
        Expanded(
            flex: 3,
            child: FittedBox(
                fit: BoxFit.scaleDown,
                alignment: Alignment.centerRight,
                child: TextButton(
                    onPressed: onPressed,
                    child: Text(
                      translate(isOk ? "CLOSE" : "OPEN"),
                      style: const TextStyle(fontWeight: FontWeight.bold),
                    )))),
      ],
    );
  }
}

class ConnectionManager extends StatelessWidget {
  const ConnectionManager({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    return Column(
        children: serverModel.clients
            .map((client) => PaddingCard(
                title: translate(client.isFileTransfer
                    ? "File Connection"
                    : "Screen Connection"),
                titleIcon: client.isFileTransfer
                    ? Icons.folder_outlined
                    : Icons.mobile_screen_share,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Expanded(child: ClientInfo(client)),
                        Expanded(
                            flex: -1,
                            child: client.isFileTransfer || !client.authorized
                                ? const SizedBox.shrink()
                                : IconButton(
                                    onPressed: () {
                                      gFFI.chatModel.changeCurrentID(client.id);
                                      final bar =
                                          navigationBarKey.currentWidget;
                                      if (bar != null) {
                                        bar as BottomNavigationBar;
                                        bar.onTap!(1);
                                      }
                                    },
                                    icon: const Icon(
                                      Icons.chat,
                                      color: MyTheme.accent,
                                    )))
                      ],
                    ),
                    client.authorized
                        ? const SizedBox.shrink()
                        : Text(
                            translate("android_new_connection_tip"),
                            style: Theme.of(globalKey.currentContext!)
                                .textTheme
                                .bodyMedium,
                          ),
                    client.authorized
                        ? ElevatedButton.icon(
                            style: ButtonStyle(
                                backgroundColor:
                                    MaterialStatePropertyAll(Colors.red)),
                            icon: const Icon(Icons.close),
                            onPressed: () {
                              bind.cmCloseConnection(connId: client.id);
                              gFFI.invokeMethod(
                                  "cancel_notification", client.id);
                            },
                            label: Text(translate("Disconnect")))
                        : Row(children: [
                            TextButton(
                                child: Text(translate("Dismiss")),
                                onPressed: () {
                                  serverModel.sendLoginResponse(client, false);
                                }),
                            const SizedBox(width: 20),
                            ElevatedButton(
                                child: Text(translate("Accept")),
                                onPressed: () {
                                  serverModel.sendLoginResponse(client, true);
                                }),
                          ]),
                  ],
                )))
            .toList());
  }
}

class PaddingCard extends StatelessWidget {
  const PaddingCard({Key? key, required this.child, this.title, this.titleIcon})
      : super(key: key);

  final String? title;
  final IconData? titleIcon;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final children = [child];
    if (title != null) {
      children.insert(
          0,
          Padding(
              padding: const EdgeInsets.symmetric(vertical: 5.0),
              child: Row(
                children: [
                  titleIcon != null
                      ? Padding(
                          padding: const EdgeInsets.only(right: 10),
                          child:
                              Icon(titleIcon, color: MyTheme.accent, size: 30))
                      : const SizedBox.shrink(),
                  Text(
                    title!,
                    style: const TextStyle(
                      fontFamily: 'WorkSans',
                      fontWeight: FontWeight.bold,
                      fontSize: 20,
                      color: MyTheme.accent,
                    ),
                  )
                ],
              )));
    }
    return SizedBox(
        width: double.maxFinite,
        child: Card(
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(13),
          ),
          margin: const EdgeInsets.fromLTRB(12.0, 10.0, 12.0, 0),
          child: Padding(
            padding:
                const EdgeInsets.symmetric(vertical: 15.0, horizontal: 30.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: children,
            ),
          ),
        ));
  }
}

class ClientInfo extends StatelessWidget {
  final Client client;
  ClientInfo(this.client);

  @override
  Widget build(BuildContext context) {
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(
            children: [
              Expanded(
                  flex: -1,
                  child: Padding(
                      padding: const EdgeInsets.only(right: 12),
                      child: CircleAvatar(
                          backgroundColor:
                              str2color(client.name).withOpacity(0.7),
                          child: Text(client.name[0])))),
              Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                    Text(client.name,
                        style: const TextStyle(
                            color: MyTheme.idColor, fontSize: 18)),
                    const SizedBox(width: 8),
                    Text(client.peerId,
                        style: const TextStyle(
                            color: MyTheme.idColor, fontSize: 10))
                  ]))
            ],
          ),
        ]));
  }
}

void androidChannelInit() {
  gFFI.setMethodCallHandler((method, arguments) {
    debugPrint("flutter got android msg,$method,$arguments");
    try {
      switch (method) {
        case "start_capture":
          {
            gFFI.dialogManager.dismissAll();
            gFFI.serverModel.updateClientState();
            break;
          }
        case "on_state_changed":
          {
            var name = arguments["name"] as String;
            var value = arguments["value"] as String == "true";
            debugPrint("from jvm:on_state_changed,$name:$value");
            gFFI.serverModel.changeStatue(name, value);
            break;
          }
        case "on_android_permission_result":
          {
            var type = arguments["type"] as String;
            var result = arguments["result"] as bool;
            AndroidPermissionManager.complete(type, result);
            break;
          }
        case "on_media_projection_canceled":
          {
            gFFI.serverModel.stopService();
            break;
          }
      }
    } catch (e) {
      debugPrintStack(label: "MethodCallHandler err:$e");
    }
    return "";
  });
}
