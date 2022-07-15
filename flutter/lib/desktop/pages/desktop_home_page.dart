import 'dart:io';

import 'package:flutter/material.dart' hide MenuItem;
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart';
import 'package:flutter_hbb/desktop/widgets/titlebar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:tray_manager/tray_manager.dart';

class DesktopHomePage extends StatefulWidget {
  DesktopHomePage({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _DesktopHomePageState();
}

const borderColor = Color(0xFF2F65BA);

class _DesktopHomePageState extends State<DesktopHomePage> with TrayListener {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          DesktopTitleBar(
            child: Center(
              child: Text(
                "RustDesk",
                style: TextStyle(
                    color: Colors.white,
                    fontSize: 20,
                    fontWeight: FontWeight.bold),
              ),
            ),
          ),
          Expanded(
            child: Container(
              child: Row(
                children: [
                  Flexible(
                    child: buildServerInfo(context),
                    flex: 1,
                  ),
                  Flexible(
                    child: buildServerBoard(context),
                    flex: 4,
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  buildServerInfo(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: gFFI.serverModel,
      child: Container(
        decoration: BoxDecoration(color: MyTheme.white),
        child: Column(
          children: [
            buildTip(context),
            buildIDBoard(context),
            buildPasswordBoard(context),
          ],
        ),
      ),
    );
  }

  buildServerBoard(BuildContext context) {
    return Column(
      children: [
        Expanded(child: ConnectionPage()),
      ],
    );
  }

  buildIDBoard(BuildContext context) {
    final model = gFFI.serverModel;
    return Container(
      margin: EdgeInsets.symmetric(vertical: 4.0, horizontal: 16.0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Container(
            width: 3,
            height: 70,
            decoration: BoxDecoration(color: MyTheme.accent),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        translate("ID"),
                        style: TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
                      ),
                      PopupMenuButton(
                        padding: EdgeInsets.all(4.0),
                          itemBuilder: (context) => [
                            genEnablePopupMenuItem(translate("Enable Keyboard/Mouse"), 'enable-keyboard',),
                            genEnablePopupMenuItem(translate("Enable Clipboard"), 'enable-clipboard',),
                            genEnablePopupMenuItem(translate("Enable File Transfer"), 'enable-file-transfer',),
                            genEnablePopupMenuItem(translate("Enable TCP Tunneling"), 'enable-tunnel',),
                            genAudioInputPopupMenuItem(),
                            // TODO: Audio Input
                            PopupMenuItem(child: Text(translate("ID/Relay Server")), value: 'custom-server',),
                            PopupMenuItem(child: Text(translate("IP Whitelisting")), value: 'whitelist',),
                            PopupMenuItem(child: Text(translate("Socks5 Proxy")), value: 'Socks5 Proxy',),
                            // sep
                            genEnablePopupMenuItem(translate("Enable Service"), 'stop-service',),
                            // TODO: direct server
                            genEnablePopupMenuItem(translate("Always connected via relay"),'allow-always-relay',),
                            genEnablePopupMenuItem(translate("Start ID/relay service"),'stop-rendezvous-service',),
                            PopupMenuItem(child: Text(translate("Change ID")), value: 'change-id',),
                            genEnablePopupMenuItem(translate("Dark Theme"), 'allow-darktheme',),
                            PopupMenuItem(child: Text(translate("About")), value: 'about',),
                      ], onSelected: onSelectMenu,)
                    ],
                  ),
                  TextFormField(
                    controller: model.serverId,
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  buildPasswordBoard(BuildContext context) {
    final model = gFFI.serverModel;
    return Container(
      margin: EdgeInsets.symmetric(vertical: 4.0, horizontal: 16.0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Container(
            width: 3,
            height: 70,
            decoration: BoxDecoration(color: MyTheme.accent),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    translate("Password"),
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
                  ),
                  TextFormField(
                    controller: model.serverPasswd,
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  buildTip(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 16.0),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            translate("Your Desktop"),
            style: TextStyle(fontWeight: FontWeight.bold, fontSize: 20),
          ),
          SizedBox(
            height: 8.0,
          ),
          Text(
            translate("desk_tip"),
            overflow: TextOverflow.clip,
            style: TextStyle(fontSize: 14),
          )
        ],
      ),
    );
  }

  buildControlPanel(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(10), color: MyTheme.white),
      padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("Control Remote Desktop")),
          Form(
              child: Column(
            children: [
              TextFormField(
                controller: TextEditingController(),
                inputFormatters: [
                  FilteringTextInputFormatter.allow(RegExp(r"[0-9]"))
                ],
              )
            ],
          ))
        ],
      ),
    );
  }

  buildRecentSession(BuildContext context) {
    return Center(child: Text("waiting implementation"));
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    print("click ${menuItem.key}");
    switch (menuItem.key) {
      case "quit":
        exit(0);
      case "show":
        // windowManager.show();
        break;
      default:
        break;
    }
  }

  @override
  void initState() {
    super.initState();
    trayManager.addListener(this);
  }

  @override
  void dispose() {
    trayManager.removeListener(this);
    super.dispose();
  }

  void onSelectMenu(String value) {
    if (value.startsWith('enable-')) {
      final option = gFFI.getOption(value);
      gFFI.setOption(value, option == "N" ? "" : "N");
    } else if (value.startsWith('allow-')) {
      final option = gFFI.getOption(value);
      gFFI.setOption(value, option == "Y" ? "" : "Y");
    } else if (value == "stop-service") {
      final option = gFFI.getOption(value);
      gFFI.setOption(value, option == "Y" ? "" : "Y");
    } else if (value == "change-id") {
      changeId();
    }
  }

  PopupMenuItem<String> genEnablePopupMenuItem(String label, String value) {
    final isEnable =
        label.startsWith('enable-') ? gFFI.getOption(value) != "N" : gFFI.getOption(value) != "Y";
    return PopupMenuItem(child: Row(
      children: [
        Offstage(offstage: !isEnable, child: Icon(Icons.check)),
        Text(label, style: genTextStyle(isEnable),),
      ],
    ), value: value,);
  }

  TextStyle genTextStyle(bool isPositive) {
    return isPositive ? TextStyle() : TextStyle(
        color: Colors.redAccent,
        decoration: TextDecoration.lineThrough
    );
  }

  PopupMenuItem<String> genAudioInputPopupMenuItem() {
    final _enabledInput = gFFI.getOption('enable-audio');
    var defaultInput = gFFI.getDefaultAudioInput().obs;
    var enabled = (_enabledInput != "N").obs;
    return PopupMenuItem(child: FutureBuilder<List<String>>(
      future: gFFI.getAudioInputs(),
      builder: (context, snapshot) {
        if (snapshot.hasData) {
          final inputs = snapshot.data!;
          if (Platform.isWindows) {
            inputs.insert(0, translate("System Sound"));
          }
          var inputList = inputs.map((e) => PopupMenuItem(
            child: Row(
              children: [
                Obx(()=> Offstage(offstage: defaultInput.value != e, child: Icon(Icons.check))),
                Expanded(child: Tooltip(
                    message: e,
                    child: Text("$e",maxLines: 1, overflow: TextOverflow.ellipsis,))),
              ],
            ),
            value: e,
          )).toList();
          inputList.insert(0, PopupMenuItem(
            child: Row(
              children: [
                Obx(()=> Offstage(offstage: enabled.value, child: Icon(Icons.check))),
                Expanded(child: Text(translate("Mute"))),
              ],
            ),
            value: "Mute",
          ));
          return PopupMenuButton<String>(
              padding: EdgeInsets.zero,
              child: Container(
                  alignment: Alignment.centerLeft,
                  child: Text(translate("Audio Input"))),
              itemBuilder: (context) => inputList,
              onSelected: (dev) {
                  if (dev == "Mute") {
                    gFFI.setOption('enable-audio', _enabledInput == 'N' ? '': 'N');
                    enabled.value = gFFI.getOption('enable-audio') != 'N';
                  } else if (dev != gFFI.getDefaultAudioInput()) {
                    gFFI.setDefaultAudioInput(dev);
                    defaultInput.value = dev;
                  }
              },
          );
        } else {
          return Text("...");
        }
      },
    ), value: 'audio-input',);
  }

  /// change local ID
  void changeId() {
    var newId = "";
    var msg = "";
    var isInProgress = false;
    DialogManager.show( (setState, close) {
      return CustomAlertDialog(
        title: Text(translate("Change ID")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("id_change_tip")),
            SizedBox(height: 8.0,),
            Row(
              children: [
                Text("ID:").marginOnly(bottom: 16.0),
                SizedBox(width: 24.0,),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      newId = s;
                    },
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg)
                    ),
                    inputFormatters: [
                      LengthLimitingTextInputFormatter(16),
                      // FilteringTextInputFormatter(RegExp(r"[a-zA-z][a-zA-z0-9\_]*"), allow: true)
                    ],
                    maxLength: 16,
                  ),
                ),
              ],
            ),
            SizedBox(height: 4.0,),
            Offstage(
                offstage: !isInProgress,
                child: LinearProgressIndicator())
          ],
        ), actions: [
          TextButton(onPressed: (){
            close();
          }, child: Text("取消")),
          TextButton(onPressed: () async {
            setState(() {
              msg = "";
              isInProgress = true;
              gFFI.bind.mainChangeId(newId: newId);
            });

            var status = await gFFI.bind.mainGetAsyncStatus();
            while (status == " "){
              await Future.delayed(Duration(milliseconds: 100));
              status = await gFFI.bind.mainGetAsyncStatus();
            }
            setState(() {
              isInProgress = false;
              msg = translate(status);
            });

          }, child: Text("确定")),
      ],
      );
    });
  }
}
