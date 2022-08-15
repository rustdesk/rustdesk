import 'package:flutter/material.dart';
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
            builder: (context, serverModel, child) => SingleChildScrollView(
                  controller: gFFI.serverModel.controller,
                  child: Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.start,
                      children: [
                        ConnectionManager(),
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
    return Column(
        children: serverModel.clients.entries
            .map((entry) => PaddingCard(
                title: translate(entry.value.isFileTransfer
                    ? "File Connection"
                    : "Screen Connection"),
                titleIcon: entry.value.isFileTransfer
                    ? Icons.folder_outlined
                    : Icons.mobile_screen_share,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Expanded(child: clientInfo(entry.value)),
                        Expanded(
                            flex: -1,
                            child: entry.value.isFileTransfer ||
                                    !entry.value.authorized
                                ? SizedBox.shrink()
                                : IconButton(
                                    onPressed: () {
                                      gFFI.chatModel
                                          .changeCurrentID(entry.value.id);
                                      final bar =
                                          navigationBarKey.currentWidget;
                                      if (bar != null) {
                                        bar as BottomNavigationBar;
                                        bar.onTap!(1);
                                      }
                                    },
                                    icon: Icon(
                                      Icons.chat,
                                      color: MyTheme.accent80,
                                    )))
                      ],
                    ),
                    entry.value.authorized
                        ? SizedBox.shrink()
                        : Text(
                            translate("android_new_connection_tip"),
                            style: TextStyle(color: Colors.black54),
                          ),
                    entry.value.authorized
                        ? ElevatedButton.icon(
                            style: ButtonStyle(
                                backgroundColor:
                                    MaterialStateProperty.all(Colors.red)),
                            icon: Icon(Icons.close),
                            onPressed: () {
                              bind.serverCloseConnection(connId: entry.key);
                              gFFI.invokeMethod(
                                  "cancel_notification", entry.key);
                            },
                            label: Text(translate("Close")))
                        : Row(children: [
                            TextButton(
                                child: Text(translate("Dismiss")),
                                onPressed: () {
                                  serverModel.sendLoginResponse(
                                      entry.value, false);
                                }),
                            SizedBox(width: 20),
                            ElevatedButton(
                                child: Text(translate("Accept")),
                                onPressed: () {
                                  serverModel.sendLoginResponse(
                                      entry.value, true);
                                }),
                          ]),
                  ],
                )))
            .toList());
  }
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
