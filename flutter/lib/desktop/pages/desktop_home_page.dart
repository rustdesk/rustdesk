import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart';
import 'package:flutter_hbb/desktop/widgets/titlebar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';

class DesktopHomePage extends StatefulWidget {
  DesktopHomePage({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _DesktopHomePageState();
}

const borderColor = Color(0xFF2F65BA);

class _DesktopHomePageState extends State<DesktopHomePage> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          Row(
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
              )
            ],
          ),
          Expanded(
            child: Container(
              child: Row(
                children: [
                  Flexible(
                    child: buildServerInfo(context),
                    flex: 1,
                  ),
                  SizedBox(
                    width: 16.0,
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
      value: FFI.serverModel,
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
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // buildControlPanel(context),
        // buildRecentSession(context),
        ConnectionPage()
      ],
    );
  }

  buildIDBoard(BuildContext context) {
    final model = FFI.serverModel;
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
                    translate("ID"),
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
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
    final model = FFI.serverModel;
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
                inputFormatters: [],
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
}
