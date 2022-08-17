import 'package:flutter/material.dart';
import 'package:get/get.dart';
// import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:provider/provider.dart';

import '../../common.dart';
import '../../mobile/pages/home_page.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';

class DesktopServerPage extends StatefulWidget implements PageShape {
  @override
  final title = translate("Share Screen");

  @override
  final icon = Icon(Icons.mobile_screen_share);

  @override
  final appBarActions = [
    PopupMenuButton<String>(
        icon: Icon(Icons.more_vert),
        itemBuilder: (context) {
          return [
            PopupMenuItem(
              child: Text(translate("Change ID")),
              padding: EdgeInsets.symmetric(horizontal: 16.0),
              value: "changeID",
              enabled: false,
            ),
            PopupMenuItem(
              child: Text(translate("Set permanent password")),
              padding: EdgeInsets.symmetric(horizontal: 16.0),
              value: "setPermanentPassword",
              enabled:
                  gFFI.serverModel.verificationMethod != kUseTemporaryPassword,
            ),
            PopupMenuItem(
              child: Text(translate("Set temporary password length")),
              padding: EdgeInsets.symmetric(horizontal: 16.0),
              value: "setTemporaryPasswordLength",
              enabled:
                  gFFI.serverModel.verificationMethod != kUsePermanentPassword,
            ),
            const PopupMenuDivider(),
            PopupMenuItem(
              padding: EdgeInsets.symmetric(horizontal: 0.0),
              value: kUseTemporaryPassword,
              child: Container(
                  child: ListTile(
                      title: Text(translate("Use temporary password")),
                      trailing: Icon(
                        Icons.check,
                        color: gFFI.serverModel.verificationMethod ==
                                kUseTemporaryPassword
                            ? null
                            : Color(0xFFFFFFFF),
                      ))),
            ),
            PopupMenuItem(
              padding: EdgeInsets.symmetric(horizontal: 0.0),
              value: kUsePermanentPassword,
              child: ListTile(
                  title: Text(translate("Use permanent password")),
                  trailing: Icon(
                    Icons.check,
                    color: gFFI.serverModel.verificationMethod ==
                            kUsePermanentPassword
                        ? null
                        : Color(0xFFFFFFFF),
                  )),
            ),
            PopupMenuItem(
              padding: EdgeInsets.symmetric(horizontal: 0.0),
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
                        : Color(0xFFFFFFFF),
                  )),
            ),
          ];
        },
        onSelected: (value) {
          if (value == "changeID") {
            // TODO
          } else if (value == "setPermanentPassword") {
            // setPermanentPasswordDialog();
          } else if (value == "setTemporaryPasswordLength") {
            // setTemporaryPasswordLengthDialog();
          } else if (value == kUsePermanentPassword ||
              value == kUseTemporaryPassword ||
              value == kUseBothPasswords) {
            bind.mainSetOption(key: "verification-method", value: value);
            gFFI.serverModel.updatePasswordModel();
          }
        })
  ];

  @override
  State<StatefulWidget> createState() => _DesktopServerPageState();
}

class _DesktopServerPageState extends State<DesktopServerPage> {
  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(
            builder: (context, serverModel, child) => Material(
                  child: Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.start,
                      children: [
                        Expanded(child: ConnectionManager()),
                        SizedBox.fromSize(size: Size(0, 15.0)),
                      ],
                    ),
                  ),
                )));
  }
}

class ConnectionManager extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    // test case:
    // serverModel.clients.clear();
    // serverModel.clients[0] = Client(false, false, "Readmi-M21sdfsdf", "123123123", true, false, false);
    return DefaultTabController(
      length: serverModel.clients.length,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            height: kTextTabBarHeight,
            child: TabBar(
                isScrollable: true,
                tabs: serverModel.clients.entries
                    .map((entry) => buildTab(entry))
                    .toList(growable: false)),
          ),
          Expanded(
            child: TabBarView(
                children: serverModel.clients.entries
                    .map((entry) => buildConnectionCard(entry))
                    .toList(growable: false)),
          )
        ],
      ),
    );
  }

  Widget buildConnectionCard(MapEntry<int, Client> entry) {
    final client = entry.value;
    return Column(
      children: [
        _CmHeader(client: client),
        _PrivilegeBoard(client: client),
        Expanded(
            child: Align(
          alignment: Alignment.bottomCenter,
          child: _CmControlPanel(client: client),
        ))
      ],
    ).paddingSymmetric(vertical: 8.0, horizontal: 8.0);
  }

  Widget buildTab(MapEntry<int, Client> entry) {
    return Tab(
      child: Row(
        children: [
          SizedBox(
              width: 80,
              child: Text(
                "${entry.value.name}",
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
              )),
        ],
      ),
    );
  }
}

class _CmHeader extends StatelessWidget {
  final Client client;

  const _CmHeader({Key? key, required this.client}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // icon
        Container(
          width: 100,
          height: 100,
          alignment: Alignment.center,
          decoration: BoxDecoration(color: str2color(client.name)),
          child: Text(
            "${client.name[0]}",
            style: TextStyle(
                fontWeight: FontWeight.bold, color: Colors.white, fontSize: 75),
          ),
        ).marginOnly(left: 4.0, right: 8.0),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                "${client.name}",
                style: TextStyle(
                  color: MyTheme.cmIdColor,
                  fontWeight: FontWeight.bold,
                  fontSize: 20,
                  overflow: TextOverflow.ellipsis,
                ),
                maxLines: 1,
              ),
              Text("(${client.peerId})",
                  style: TextStyle(color: MyTheme.cmIdColor, fontSize: 14)),
              SizedBox(
                height: 16.0,
              ),
              Offstage(
                  offstage: !client.authorized,
                  child: Row(
                    children: [
                      Text("${translate("Connected")}"),
                    ],
                  ))
            ],
          ),
        ),
        Offstage(
          offstage: client.isFileTransfer,
          child: IconButton(
            onPressed: handleSendMsg,
            icon: Icon(Icons.message_outlined),
          ),
        )
      ],
    );
  }

  void handleSendMsg() {}
}

class _PrivilegeBoard extends StatelessWidget {
  final Client client;

  const _PrivilegeBoard({Key? key, required this.client}) : super(key: key);

  Widget buildPermissionIcon(bool enabled, ImageProvider icon,
      Function(bool)? onTap, String? tooltip) {
    return Tooltip(
      message: tooltip ?? "",
      child: Ink(
        decoration:
            BoxDecoration(color: enabled ? MyTheme.accent80 : Colors.grey),
        padding: EdgeInsets.all(4.0),
        child: InkWell(
          onTap: () => onTap?.call(!enabled),
          child: Image(
            image: icon,
            width: 50,
            height: 50,
            fit: BoxFit.scaleDown,
          ),
        ),
      ).marginSymmetric(horizontal: 4.0),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: EdgeInsets.only(top: 16.0, bottom: 8.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            translate("Permissions"),
            style: TextStyle(fontSize: 16),
          ).marginOnly(left: 4.0),
          SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              buildPermissionIcon(
                  client.keyboard, iconKeyboard, (enable) => null, null),
              buildPermissionIcon(
                  client.clipboard, iconClipboard, (enable) => null, null),
              buildPermissionIcon(
                  client.audio, iconAudio, (enable) => null, null),
              // TODO: file transfer
              buildPermissionIcon(false, iconFile, (enable) => null, null),
            ],
          ),
        ],
      ),
    );
  }
}

class _CmControlPanel extends StatelessWidget {
  final Client client;

  const _CmControlPanel({Key? key, required this.client}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return client.authorized ? buildAuthorized() : buildUnAuthorized();
  }

  buildAuthorized() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Ink(
          width: 200,
          height: 40,
          decoration: BoxDecoration(
              color: Colors.redAccent, borderRadius: BorderRadius.circular(10)),
          child: InkWell(
              onTap: handleDisconnect,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    translate("Disconnect"),
                    style: TextStyle(color: Colors.white),
                  ),
                ],
              )),
        )
      ],
    );
  }

  buildUnAuthorized() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Ink(
          width: 100,
          height: 40,
          decoration: BoxDecoration(
              color: MyTheme.accent, borderRadius: BorderRadius.circular(10)),
          child: InkWell(
              onTap: handleAccept,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    translate("Accept"),
                    style: TextStyle(color: Colors.white),
                  ),
                ],
              )),
        ),
        SizedBox(
          width: 30,
        ),
        Ink(
          width: 100,
          height: 40,
          decoration: BoxDecoration(
              color: Colors.transparent,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: Colors.grey)),
          child: InkWell(
              onTap: handleCancel,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    translate("Cancel"),
                    style: TextStyle(),
                  ),
                ],
              )),
        )
      ],
    );
  }

  void handleDisconnect() {}

  void handleCancel() {}

  void handleAccept() {}
}

class PaddingCard extends StatelessWidget {
  PaddingCard({required this.child, this.title, this.titleIcon});

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
              padding: EdgeInsets.symmetric(vertical: 5.0),
              child: Row(
                children: [
                  titleIcon != null
                      ? Padding(
                          padding: EdgeInsets.only(right: 10),
                          child: Icon(titleIcon,
                              color: MyTheme.accent80, size: 30))
                      : SizedBox.shrink(),
                  Text(
                    title!,
                    style: TextStyle(
                      fontFamily: 'WorkSans',
                      fontWeight: FontWeight.bold,
                      fontSize: 20,
                      color: MyTheme.accent80,
                    ),
                  )
                ],
              )));
    }
    return Container(
        width: double.maxFinite,
        child: Card(
          margin: EdgeInsets.fromLTRB(15.0, 15.0, 15.0, 0),
          child: Padding(
            padding: EdgeInsets.symmetric(vertical: 15.0, horizontal: 30.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: children,
            ),
          ),
        ));
  }
}

Widget clientInfo(Client client) {
  return Padding(
      padding: EdgeInsets.symmetric(vertical: 8),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(
          children: [
            Expanded(
                flex: -1,
                child: Padding(
                    padding: EdgeInsets.only(right: 12),
                    child: CircleAvatar(
                        child: Text(client.name[0]),
                        backgroundColor: MyTheme.border))),
            Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                  Text(client.name,
                      style: TextStyle(color: MyTheme.idColor, fontSize: 18)),
                  SizedBox(width: 8),
                  Text(client.peerId,
                      style: TextStyle(color: MyTheme.idColor, fontSize: 10))
                ]))
          ],
        ),
      ]));
}
