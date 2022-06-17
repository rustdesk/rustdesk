import 'package:flutter/material.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';
import '../../mobile/pages/home_page.dart';
import '../../mobile/pages/scan_page.dart';
import '../../mobile/pages/settings_page.dart';
import '../../models/model.dart';

/// Connection page for connecting to a remote peer.
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

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage> {
  /// Controller for the id input bar.
  final _idController = TextEditingController();

  /// Update url. If it's not null, means an update is available.
  var _updateUrl = '';
  var _menuPos;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    if (_idController.text.isEmpty) _idController.text = gFFI.getId();
    return SingleChildScrollView(
      child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          mainAxisSize: MainAxisSize.max,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            getUpdateUI(),
            getSearchBarUI(),
            SizedBox(height: 12),
            getPeers(),
          ]),
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    var id = _idController.text.trim();
    connect(id, isFileTransfer: isFileTransfer);
  }

  /// Connect to a peer with [id].
  /// If [isFileTransfer], starts a session only for file transfer.
  void connect(String id, {bool isFileTransfer = false}) async {
    if (id == '') return;
    id = id.replaceAll(' ', '');
    if (isFileTransfer) {
      if (!isDesktop) {
        if (!await PermissionManager.check("file")) {
          if (!await PermissionManager.request("file")) {
            return;
          }
        }
      }
      await rustDeskWinManager.new_file_transfer(id);
    } else {
      await rustDeskWinManager.new_remote_desktop(id);
    }
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  /// UI for software update.
  /// If [_updateUrl] is not empty, shows a button to update the software.
  Widget getUpdateUI() {
    return _updateUrl.isEmpty
        ? SizedBox(height: 0)
        : InkWell(
            onTap: () async {
              final url = _updateUrl + '.apk';
              if (await canLaunch(url)) {
                await launch(url);
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

  /// UI for the search bar.
  /// Search for a peer and connect to it if the id exists.
  Widget getSearchBarUI() {
    var w = Padding(
      padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 16.0),
      child: Container(
        child: Padding(
          padding: const EdgeInsets.only(top: 16, bottom: 16),
          child: Ink(
            decoration: BoxDecoration(
              color: MyTheme.white,
              borderRadius: const BorderRadius.all(Radius.circular(13)),
            ),
            child: Column(
              children: [
                Row(
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
                            // color: MyTheme.idColor,
                          ),
                          decoration: InputDecoration(
                            labelText: translate('Control Remote Desktop'),
                            // hintText: 'Enter your remote ID',
                            // border: InputBorder.,
                            border: OutlineInputBorder(
                                borderRadius: BorderRadius.zero),
                            helperStyle: TextStyle(
                              fontWeight: FontWeight.bold,
                              fontSize: 16,
                              color: MyTheme.dark,
                            ),
                            labelStyle: TextStyle(
                              fontWeight: FontWeight.w600,
                              fontSize: 26,
                              letterSpacing: 0.2,
                              color: MyTheme.dark,
                            ),
                          ),
                          controller: _idController,
                        ),
                      ),
                    ),
                  ],
                ),
                Padding(
                  padding: const EdgeInsets.symmetric(
                      vertical: 16.0, horizontal: 16.0),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      OutlinedButton(
                        onPressed: () {
                          onConnect(isFileTransfer: true);
                        },
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                              vertical: 8.0, horizontal: 8.0),
                          child: Text(
                            translate(
                              "File Transfer",
                            ),
                            style: TextStyle(color: MyTheme.dark),
                          ),
                        ),
                      ),
                      SizedBox(
                        width: 30,
                      ),
                      OutlinedButton(
                        onPressed: onConnect,
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                              vertical: 8.0, horizontal: 16.0),
                          child: Text(
                            translate(
                              "Connection",
                            ),
                            style: TextStyle(color: MyTheme.white),
                          ),
                        ),
                        style: OutlinedButton.styleFrom(
                          backgroundColor: Colors.blueAccent,
                        ),
                      ),
                    ],
                  ),
                )
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

  /// Get the image for the current [platform].
  Widget getPlatformImage(String platform) {
    platform = platform.toLowerCase();
    if (platform == 'mac os')
      platform = 'mac';
    else if (platform != 'linux' && platform != 'android') platform = 'win';
    return Image.asset('assets/$platform.png', width: 24, height: 24);
  }

  /// Get all the saved peers.
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
    var peers = gFFI.peers();
    peers.forEach((p) {
      cards.add(Container(
          width: width,
          child: Card(
              child: GestureDetector(
                  onTap: !isWebDesktop ? () => connect('${p.id}') : null,
                  onDoubleTap: isWebDesktop ? () => connect('${p.id}') : null,
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

  /// Show the peer menu and handle user's choice.
  /// User might remove the peer or send a file to the peer.
  void showPeerMenu(BuildContext context, String id) async {
    var value = await showMenu(
      context: context,
      position: this._menuPos,
      items: [
            PopupMenuItem<String>(
                child: Text(translate('Remove')), value: 'remove')
          ] +
          ([
            PopupMenuItem<String>(
                child: Text(translate('File transfer')), value: 'file')
          ]),
      elevation: 8,
    );
    if (value == 'remove') {
      setState(() => gFFI.setByName('remove', '$id'));
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
