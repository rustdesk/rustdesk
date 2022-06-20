import 'package:flutter/material.dart';
import 'package:flutter_hbb/pages/file_manager_page.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'dart:async';
import '../common.dart';
import '../models/model.dart';
import 'home_page.dart';
import 'remote_page.dart';
import 'settings_page.dart';
import 'scan_page.dart';

class ConnectionPage extends StatefulWidget implements PageShape {
  ConnectionPage({Key? key}) : super(key: key);

  @override
  final icon = Icon(Icons.connected_tv);

  @override
  final title = translate("Connection");

  @override
  final appBarActions = !isAndroid ? <Widget>[WebMenu()] : <Widget>[];

  @override
  _ConnectionPageState createState() => _ConnectionPageState();
}

class _ConnectionPageState extends State<ConnectionPage> {
  final _idController = TextEditingController();
  var _updateUrl = '';
  var _menuPos;

  @override
  void initState() {
    super.initState();
    if (isAndroid) {
      Timer(Duration(seconds: 5), () {
        _updateUrl = FFI.getByName('software_update_url');
        if (_updateUrl.isNotEmpty) setState(() {});
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    if (_idController.text.isEmpty) _idController.text = FFI.getId();
    return SingleChildScrollView(
      child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          mainAxisSize: MainAxisSize.max,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            getUpdateUI(),
            getSearchBarUI(),
            Container(height: 12),
            getPeers(),
          ]),
    );
  }

  void onConnect() {
    var id = _idController.text.trim();
    connect(id);
  }

  void connect(String id, {bool isFileTransfer = false}) async {
    if (id == '') return;
    id = id.replaceAll(' ', '');
    if (isFileTransfer) {
      if (!await PermissionManager.check("file")) {
        if (!await PermissionManager.request("file")) {
          return;
        }
      }
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => FileManagerPage(id: id),
        ),
      );
    } else {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => RemotePage(id: id),
        ),
      );
    }
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  Widget getUpdateUI() {
    return _updateUrl.isEmpty
        ? SizedBox(height: 0)
        : InkWell(
            onTap: () async {
              final url = _updateUrl + '.apk';
              if (await canLaunchUrl(Uri.parse(url))) {
                await launchUrl(Uri.parse(url));
              }
            },
            child: Container(
                alignment: AlignmentDirectional.center,
                width: double.infinity,
                color: Colors.pinkAccent,
                padding: EdgeInsets.symmetric(vertical: 12),
                child: Text(translate('Download new version'),
                    style: TextStyle(
                        color: Colors.white, fontWeight: FontWeight.bold))));
  }

  Widget getSearchBarUI() {
    var w = Padding(
      padding: const EdgeInsets.fromLTRB(16.0, 8.0, 16.0, 0.0),
      child: Container(
        height: 84,
        child: Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 8),
          child: Ink(
            decoration: BoxDecoration(
              color: MyTheme.white,
              borderRadius: const BorderRadius.all(Radius.circular(13)),
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
                      style: TextStyle(
                        fontFamily: 'WorkSans',
                        fontWeight: FontWeight.bold,
                        fontSize: 30,
                        color: MyTheme.idColor,
                      ),
                      decoration: InputDecoration(
                        labelText: translate('Remote ID'),
                        // hintText: 'Enter your remote ID',
                        border: InputBorder.none,
                        helperStyle: TextStyle(
                          fontWeight: FontWeight.bold,
                          fontSize: 16,
                          color: MyTheme.darkGray,
                        ),
                        labelStyle: TextStyle(
                          fontWeight: FontWeight.w600,
                          fontSize: 16,
                          letterSpacing: 0.2,
                          color: MyTheme.darkGray,
                        ),
                      ),
                      controller: _idController,
                    ),
                  ),
                ),
                SizedBox(
                  width: 60,
                  height: 60,
                  child: IconButton(
                    icon: Icon(Icons.arrow_forward,
                        color: MyTheme.darkGray, size: 45),
                    onPressed: onConnect,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    return Center(
        child: Container(constraints: BoxConstraints(maxWidth: 600), child: w));
  }

  @override
  void dispose() {
    _idController.dispose();
    super.dispose();
  }

  Widget getPlatformImage(String platform) {
    platform = platform.toLowerCase();
    if (platform == 'mac os')
      platform = 'mac';
    else if (platform != 'linux' && platform != 'android') platform = 'win';
    return Image.asset('assets/$platform.png', width: 24, height: 24);
  }

  Widget getPeers() {
    final size = MediaQuery.of(context).size;
    final space = 8.0;
    var width = size.width - 2 * space;
    final minWidth = 320.0;
    if (size.width > minWidth + 2 * space) {
      final n = (size.width / (minWidth + 2 * space)).floor();
      width = size.width / n - 2 * space;
    }
    final cards = <Widget>[];
    var peers = FFI.peers();
    peers.forEach((p) {
      cards.add(Container(
          width: width,
          child: Card(
              child: GestureDetector(
                  onTap: !isDesktop ? () => connect('${p.id}') : null,
                  onDoubleTap: isDesktop ? () => connect('${p.id}') : null,
                  onLongPressStart: (details) {
                    final x = details.globalPosition.dx;
                    final y = details.globalPosition.dy;
                    _menuPos = RelativeRect.fromLTRB(x, y, x, y);
                    showPeerMenu(context, p.id);
                  },
                  child: ListTile(
                    contentPadding: const EdgeInsets.only(left: 12),
                    subtitle: Text('${p.username}@${p.hostname}'),
                    title: Text('${p.id}'),
                    leading: Container(
                        padding: const EdgeInsets.all(6),
                        child: getPlatformImage('${p.platform}'),
                        color: str2color('${p.id}${p.platform}', 0x7f)),
                    trailing: InkWell(
                        child: Padding(
                            padding: const EdgeInsets.all(12),
                            child: Icon(Icons.more_vert)),
                        onTapDown: (e) {
                          final x = e.globalPosition.dx;
                          final y = e.globalPosition.dy;
                          _menuPos = RelativeRect.fromLTRB(x, y, x, y);
                        },
                        onTap: () {
                          showPeerMenu(context, p.id);
                        }),
                  )))));
    });
    return Wrap(children: cards, spacing: space, runSpacing: space);
  }

  void showPeerMenu(BuildContext context, String id) async {
    var value = await showMenu(
      context: context,
      position: this._menuPos,
      items: [
            PopupMenuItem<String>(
                child: Text(translate('Remove')), value: 'remove')
          ] +
          (!isAndroid
              ? []
              : [
                  PopupMenuItem<String>(
                      child: Text(translate('Transfer File')), value: 'file')
                ]),
      elevation: 8,
    );
    if (value == 'remove') {
      setState(() => FFI.setByName('remove', '$id'));
      () async {
        removePreference(id);
      }();
    } else if (value == 'file') {
      connect(id, isFileTransfer: true);
    }
  }
}

class WebMenu extends StatefulWidget {
  @override
  _WebMenuState createState() => _WebMenuState();
}

class _WebMenuState extends State<WebMenu> {
  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    final username = getUsername();
    return PopupMenuButton<String>(
        icon: Icon(Icons.more_vert),
        itemBuilder: (context) {
          return (isIOS
                  ? [
                      PopupMenuItem(
                        child: Icon(Icons.qr_code_scanner, color: Colors.black),
                        value: "scan",
                      )
                    ]
                  : <PopupMenuItem<String>>[]) +
              [
                PopupMenuItem(
                  child: Text(translate('ID/Relay Server')),
                  value: "server",
                )
              ] +
              (getUrl().contains('admin.rustdesk.com')
                  ? <PopupMenuItem<String>>[]
                  : [
                      PopupMenuItem(
                        child: Text(username == null
                            ? translate("Login")
                            : translate("Logout") + ' ($username)'),
                        value: "login",
                      )
                    ]) +
              [
                PopupMenuItem(
                  child: Text(translate('About') + ' RustDesk'),
                  value: "about",
                )
              ];
        },
        onSelected: (value) {
          if (value == 'server') {
            showServerSettings();
          }
          if (value == 'about') {
            showAbout();
          }
          if (value == 'login') {
            if (username == null) {
              showLogin();
            } else {
              logout();
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
