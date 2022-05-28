import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';

class DesktopHomePage extends StatefulWidget {
  DesktopHomePage({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _DesktopHomePageState();
}

class _DesktopHomePageState extends State<DesktopHomePage> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
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
    );
  }

  buildServerInfo(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: FFI.serverModel,
      child: Column(
        children: [buildIDBoard(context)],
      ),
    );
  }

  buildServerBoard(BuildContext context) {
    return Center(
      child: ConnectionPage(key: null),
    );
  }

  buildIDBoard(BuildContext context) {
    final model = FFI.serverModel;
    return Card(
      elevation: 0.5,
      child: Container(
        margin: EdgeInsets.symmetric(vertical: 8.0, horizontal: 16.0),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.baseline,
          textBaseline: TextBaseline.alphabetic,
          children: [
            Container(
              width: 4,
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
                      style:
                          TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
                    ),
                    TextFormField(
                      controller: model.serverId,
                    ),
                    Text(
                      translate("Password"),
                      style:
                          TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
                    ),
                    TextField(
                      controller: model.serverPasswd,
                    )
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
