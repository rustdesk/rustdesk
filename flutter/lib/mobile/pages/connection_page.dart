import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';
import '../../common/widgets/login.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../consts.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import 'home_page.dart';
import 'scan_page.dart';
import 'settings_page.dart';

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget implements PageShape {
  ConnectionPage({Key? key}) : super(key: key);

  @override
  final icon = const Icon(Icons.connected_tv);

  @override
  final title = translate("Connection");

  @override
  final appBarActions = !isAndroid ? <Widget>[const WebMenu()] : <Widget>[];

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage> {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();

  /// Update url. If it's not null, means an update is available.
  var _updateUrl = '';

  @override
  void initState() {
    super.initState();
    if (_idController.text.isEmpty) {
      () async {
        final lastRemoteId = await bind.mainGetLastRemoteId();
        if (lastRemoteId != _idController.id) {
          setState(() {
            _idController.id = lastRemoteId;
          });
        }
      }();
    }
    if (isAndroid) {
      Timer(const Duration(seconds: 5), () async {
        _updateUrl = await bind.mainGetSoftwareUpdateUrl();
        if (_updateUrl.isNotEmpty) setState(() {});
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    return CustomScrollView(
      slivers: [
        SliverList(
            delegate: SliverChildListDelegate([
          _buildUpdateUI(),
          _buildRemoteIDTextField(),
        ])),
        SliverFillRemaining(
          hasScrollBody: false,
          child: PeerTabPage(),
        )
      ],
    ).marginOnly(top: 2, left: 10, right: 10);
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect() {
    var id = _idController.id;
    connect(context, id);
  }

  /// UI for software update.
  /// If [_updateUrl] is not empty, shows a button to update the software.
  Widget _buildUpdateUI() {
    return _updateUrl.isEmpty
        ? const SizedBox(height: 0)
        : InkWell(
            onTap: () async {
              final url = '$_updateUrl.apk';
              if (await canLaunchUrl(Uri.parse(url))) {
                await launchUrl(Uri.parse(url));
              }
            },
            child: Container(
                alignment: AlignmentDirectional.center,
                width: double.infinity,
                color: Colors.pinkAccent,
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: Text(translate('Download new version'),
                    style: const TextStyle(
                        color: Colors.white, fontWeight: FontWeight.bold))));
  }

  /// UI for the remote ID TextField.
  /// Search for a peer and connect to it if the id exists.
  Widget _buildRemoteIDTextField() {
    final w = SizedBox(
      height: 84,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 2),
        child: Ink(
          decoration: BoxDecoration(
            color: Theme.of(context).cardColor,
            borderRadius: BorderRadius.all(Radius.circular(13)),
          ),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Container(
                  padding: const EdgeInsets.only(left: 16, right: 16),
                  child: TextField(
                    autocorrect: false,
                    enableSuggestions: false,
                    keyboardType: TextInputType.visiblePassword,
                    // keyboardType: TextInputType.number,
                    style: const TextStyle(
                      fontFamily: 'WorkSans',
                      fontWeight: FontWeight.bold,
                      fontSize: 30,
                      color: MyTheme.idColor,
                    ),
                    decoration: InputDecoration(
                      labelText: translate('Remote ID'),
                      // hintText: 'Enter your remote ID',
                      border: InputBorder.none,
                      helperStyle: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontSize: 16,
                        color: MyTheme.darkGray,
                      ),
                      labelStyle: const TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 16,
                        letterSpacing: 0.2,
                        color: MyTheme.darkGray,
                      ),
                    ),
                    controller: _idController,
                    inputFormatters: [IDTextInputFormatter()],
                  ),
                ),
              ),
              SizedBox(
                width: 60,
                height: 60,
                child: IconButton(
                  icon: const Icon(Icons.arrow_forward,
                      color: MyTheme.darkGray, size: 45),
                  onPressed: onConnect,
                ),
              ),
            ],
          ),
        ),
      ),
    );
    return Align(
        alignment: Alignment.topLeft,
        child: Container(constraints: kMobilePageConstraints, child: w));
  }

  @override
  void dispose() {
    _idController.dispose();
    super.dispose();
  }
}

class WebMenu extends StatefulWidget {
  const WebMenu({Key? key}) : super(key: key);

  @override
  State<WebMenu> createState() => _WebMenuState();
}

class _WebMenuState extends State<WebMenu> {
  String url = "";

  @override
  void initState() {
    super.initState();
    () async {
      final urlRes = await bind.mainGetApiServer();
      var update = false;
      if (urlRes != url) {
        url = urlRes;
        update = true;
      }

      if (update) {
        setState(() {});
      }
    }();
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    return PopupMenuButton<String>(
        icon: const Icon(Icons.more_vert),
        itemBuilder: (context) {
          return (isIOS
                  ? [
                      const PopupMenuItem(
                        value: "scan",
                        child: Icon(Icons.qr_code_scanner, color: Colors.black),
                      )
                    ]
                  : <PopupMenuItem<String>>[]) +
              [
                PopupMenuItem(
                  value: "server",
                  child: Text(translate('ID/Relay Server')),
                )
              ] +
              (url.contains('admin.rustdesk.com')
                  ? <PopupMenuItem<String>>[]
                  : [
                      PopupMenuItem(
                        value: "login",
                        child: Text(gFFI.userModel.userName.value.isEmpty
                            ? translate("Login")
                            : '${translate("Logout")} (${gFFI.userModel.userName.value})'),
                      )
                    ]) +
              [
                PopupMenuItem(
                  value: "about",
                  child: Text('${translate('About')} RustDesk'),
                )
              ];
        },
        onSelected: (value) {
          if (value == 'server') {
            showServerSettings(gFFI.dialogManager);
          }
          if (value == 'about') {
            showAbout(gFFI.dialogManager);
          }
          if (value == 'login') {
            if (gFFI.userModel.userName.value.isEmpty) {
              loginDialog();
            } else {
              gFFI.userModel.logOut();
            }
          }
          if (value == 'scan') {
            Navigator.push(
              context,
              MaterialPageRoute(
                builder: (BuildContext context) => ScanPage(),
              ),
            );
          }
        });
  }
}
