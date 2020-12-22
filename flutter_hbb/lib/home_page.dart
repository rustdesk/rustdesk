import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:tuple/tuple.dart';
import 'package:package_info/package_info.dart';
import 'package:url_launcher/url_launcher.dart';
import 'dart:async';
import 'common.dart';
import 'model.dart';
import 'remote_page.dart';

class HomePage extends StatefulWidget {
  HomePage({Key key, this.title}) : super(key: key);

  final String title;

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  final _idController = TextEditingController();
  var _updateUrl = '';

  @override
  void initState() {
    super.initState();
    Timer(Duration(seconds: 5), () {
      _updateUrl = FFI.getByName('software_update_url');
      if (_updateUrl.isNotEmpty) setState(() {});
    });
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    if (_idController.text.isEmpty) _idController.text = FFI.getId();
    // This method is rerun every time setState is called
    return Scaffold(
        backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          centerTitle: true,
          actions: [
            IconButton(
                icon: Icon(Icons.more_vert),
                onPressed: () {
                  () async {
                    var value = await showMenu(
                      context: context,
                      position: RelativeRect.fromLTRB(3000, 70, 3000, 70),
                      items: [
                        PopupMenuItem<String>(
                            child: Text('ID/Relay Server'), value: 'server'),
                        PopupMenuItem<String>(
                            child: Text('About RustDesk'), value: 'about'),
                      ],
                      elevation: 8,
                    );
                    if (value == 'server') {
                      showServer(context);
                    } else if (value == 'about') {
                      showAbout(context);
                    }
                  }();
                })
          ],
          title: Text(widget.title),
        ),
        body: SingleChildScrollView(
          child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              mainAxisSize: MainAxisSize.max,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
                _updateUrl.isEmpty
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
                            child: Text('Download new version',
                                style: TextStyle(
                                    color: Colors.white,
                                    fontWeight: FontWeight.bold)))),
                getSearchBarUI(),
                getPeers(),
              ]),
        ));
  }

  void onConnect() {
    var id = _idController.text.trim();
    connect(id);
  }

  void connect(String id) {
    if (id == '') return;
    () async {
      await Navigator.push<dynamic>(
        context,
        MaterialPageRoute<dynamic>(
          builder: (BuildContext context) => RemotePage(id: id),
        ),
      );
      setState(() {});
    }();
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  Widget getSearchBarUI() {
    if (!FFI.ffiModel.initialized) {
      return Container();
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(16.0, 8.0, 16.0, 0.0),
      child: Container(
        height: 84,
        child: Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 8),
          child: Container(
            decoration: BoxDecoration(
              color: MyTheme.white,
              borderRadius: const BorderRadius.only(
                bottomRight: Radius.circular(13.0),
                bottomLeft: Radius.circular(13.0),
                topLeft: Radius.circular(13.0),
                topRight: Radius.circular(13.0),
              ),
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
                        color: Color(0xFF00B6F0),
                      ),
                      decoration: InputDecoration(
                        labelText: 'Remote ID',
                        // hintText: 'Enter your remote ID',
                        border: InputBorder.none,
                        helperStyle: TextStyle(
                          fontWeight: FontWeight.bold,
                          fontSize: 16,
                          color: Color(0xFFB9BABC),
                        ),
                        labelStyle: TextStyle(
                          fontWeight: FontWeight.w600,
                          fontSize: 16,
                          letterSpacing: 0.2,
                          color: Color(0xFFB9BABC),
                        ),
                      ),
                      autofocus: _idController.text.isEmpty,
                      controller: _idController,
                    ),
                  ),
                ),
                SizedBox(
                  width: 60,
                  height: 60,
                  child: IconButton(
                    icon: Icon(Icons.arrow_forward,
                        color: Color(0xFFB9BABC), size: 45),
                    onPressed: onConnect,
                    autofocus: _idController.text.isNotEmpty,
                  ),
                )
              ],
            ),
          ),
        ),
      ),
    );
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
    else if (platform != 'linux') platform = 'win';
    return Image.asset('assets/$platform.png', width: 24, height: 24);
  }

  Widget getPeers() {
    if (!FFI.ffiModel.initialized) {
      return Container();
    }
    final cards = <Widget>[];
    var peers = FFI.peers();
    peers.forEach((p) {
      cards.add(Padding(
          padding: EdgeInsets.symmetric(horizontal: 12),
          child: Card(
              child: GestureDetector(
                  onTap: () => connect('${p.id}'),
                  onLongPressStart: (details) {
                    var x = details.globalPosition.dx;
                    var y = details.globalPosition.dy;
                    () async {
                      var value = await showMenu(
                        context: context,
                        position: RelativeRect.fromLTRB(x, y, x, y),
                        items: [
                          PopupMenuItem<String>(
                              child: Text('Remove'), value: 'remove'),
                        ],
                        elevation: 8,
                      );
                      if (value == 'remove') {
                        setState(() => FFI.setByName('remove', '${p.id}'));
                      }
                    }();
                  },
                  child: ListTile(
                    subtitle: Text('${p.username}@${p.hostname}'),
                    title: Text('${p.id}'),
                    leading: Container(
                        padding: const EdgeInsets.all(6),
                        child: getPlatformImage('${p.platform}'),
                        color: str2color('${p.id}${p.platform}', 0x77)),
                  )))));
    });
    return Wrap(children: cards);
  }
}

void showServer(BuildContext context) {
  final formKey = GlobalKey<FormState>();
  final id0 = FFI.getByName('custom-rendezvous-server');
  final relay0 = FFI.getByName('relay-server');
  var id = '';
  var relay = '';
  showAlertDialog(
      context,
      (setState) => Tuple3(
            Text('ID/Relay Server'),
            Form(
                key: formKey,
                child:
                    Column(mainAxisSize: MainAxisSize.min, children: <Widget>[
                  TextFormField(
                    initialValue: id0,
                    decoration: InputDecoration(
                      labelText: 'ID Server',
                    ),
                    validator: validate,
                    onSaved: (String value) {
                      id = value.trim();
                    },
                  ),
                  TextFormField(
                    initialValue: relay0,
                    decoration: InputDecoration(
                      labelText: 'Relay Server',
                    ),
                    validator: validate,
                    onSaved: (String value) {
                      relay = value.trim();
                    },
                  ),
                ])),
            [
              FlatButton(
                textColor: MyTheme.accent,
                onPressed: () {
                  Navigator.pop(context);
                },
                child: Text('Cancel'),
              ),
              FlatButton(
                textColor: MyTheme.accent,
                onPressed: () {
                  if (formKey.currentState.validate()) {
                    formKey.currentState.save();
                    if (id != id0)
                      FFI.setByName('custom-rendezvous-server', id);
                    if (relay != relay0) FFI.setByName('relay-server', relay);
                    Navigator.pop(context);
                  }
                },
                child: Text('OK'),
              ),
            ],
          ));
}

Future<Null> showAbout(BuildContext context) async {
  PackageInfo packageInfo = await PackageInfo.fromPlatform();
  showAlertDialog(
      context,
      (setState) => Tuple3(
          null,
          Wrap(direction: Axis.vertical, spacing: 12, children: [
            Text('Version: ${packageInfo.version}'),
            InkWell(
                onTap: () async {
                  const url = 'https://forum.rustdesk.com/';
                  if (await canLaunch(url)) {
                    await launch(url);
                  }
                },
                child: Padding(
                  padding: EdgeInsets.symmetric(vertical: 8),
                  child: Text('Support',
                      style: TextStyle(
                        decoration: TextDecoration.underline,
                      )),
                )),
          ]),
          null),
      () async => true,
      true);
}

String validate(value) {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = FFI.getByName('test_if_valid_server', value);
  return res.isEmpty ? null : res;
}
