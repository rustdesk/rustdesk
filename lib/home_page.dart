import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:tuple/tuple.dart';
import 'package:url_launcher/url_launcher.dart';
import 'dart:async';
import 'common.dart';
import 'model.dart';
import 'remote_page.dart';

class HomePage extends StatefulWidget {
  HomePage({Key? key, required this.title}) : super(key: key);

  final String title;

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  final _idController = TextEditingController();
  var _updateUrl = '';
  var _menuPos;

  @override
  void initState() {
    super.initState();
    currentCtx = context;
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
    // This method is rerun every time setState is called
    return Scaffold(
        backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          centerTitle: true,
          actions: [
            Ink(
                child: InkWell(
                    child: Padding(
                        padding: const EdgeInsets.all(12),
                        child: Icon(Icons.more_vert)),
                    onTapDown: (e) {
                      var x = e.globalPosition.dx;
                      var y = e.globalPosition.dy;
                      this._menuPos = RelativeRect.fromLTRB(x, y, x, y);
                    },
                    onTap: () {
                      () async {
                        var value = await showMenu<dynamic>(
                          context: context,
                          position: this._menuPos,
                          items: [
                            PopupMenuItem<String>(
                                child: Text(translate('ID Server')),
                                value: 'id_server'),
                            // TODO　test
                            isAndroid
                                ? PopupMenuItem<dynamic>(
                                    child: Text(translate('Share My Screen')),
                                    value: 'server')
                                : PopupMenuItem<dynamic>(
                                    child: SizedBox.shrink(), value: ''),
                            PopupMenuItem<String>(
                                child: Text(translate('About') + ' RustDesk'),
                                value: 'about'),
                          ],
                          elevation: 8,
                        );
                        if (value == 'id_server') {
                          showServer(context);
                        } else if (value == 'server') {
                          Navigator.pushNamed(context, "server_page");
                        } else if (value == 'about') {
                          showAbout(context);
                        }
                      }();
                    }))
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
                            child: Text(translate('Download new version'),
                                style: TextStyle(
                                    color: Colors.white,
                                    fontWeight: FontWeight.bold)))),
                getSearchBarUI(),
                Container(height: 12),
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
    id = id.replaceAll(' ', '');
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
    var w = Padding(
      padding: const EdgeInsets.fromLTRB(16.0, 8.0, 16.0, 0.0),
      child: Container(
        height: 84,
        child: Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 8),
          child: Ink(
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
                        color: MyTheme.darkGray, size: 45),
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
    else if (platform != 'linux') platform = 'win';
    return Image.asset('assets/$platform.png', width: 24, height: 24);
  }

  Widget getPeers() {
    if (!FFI.ffiModel.initialized) {
      return Container();
    }
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
                  onTap: () => {
                        if (!isDesktop) {connect('${p.id}')}
                      },
                  onDoubleTap: () => {
                        if (isDesktop) {connect('${p.id}')}
                      },
                  onLongPressStart: (details) {
                    var x = details.globalPosition.dx;
                    var y = details.globalPosition.dy;
                    this._menuPos = RelativeRect.fromLTRB(x, y, x, y);
                    this.showPeerMenu(context, p.id);
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
                          var x = e.globalPosition.dx;
                          var y = e.globalPosition.dy;
                          this._menuPos = RelativeRect.fromLTRB(x, y, x, y);
                        },
                        onDoubleTap: () {},
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
            child: Text(translate('Remove')), value: 'remove'),
      ],
      elevation: 8,
    );
    if (value == 'remove') {
      setState(() => FFI.setByName('remove', '$id'));
      () async {
        removePreference(id);
      }();
    }
  }
}

void showServer(BuildContext context) {
  final formKey = GlobalKey<FormState>();
  final id0 = FFI.getByName('option', 'custom-rendezvous-server');
  final relay0 = FFI.getByName('option', 'relay-server');
  final key0 = FFI.getByName('option', 'key');
  var id = '';
  var relay = '';
  var key = '';
  showAlertDialog(
      context,
      (setState) => Tuple3(
            Text(translate('ID Server')),
            Form(
                key: formKey,
                child:
                    Column(mainAxisSize: MainAxisSize.min, children: <Widget>[
                  TextFormField(
                    initialValue: id0,
                    decoration: InputDecoration(
                      labelText: translate('ID Server'),
                    ),
                    validator: validate,
                    onSaved: (String? value) {
                      if (value != null) id = value.trim();
                    },
                  ),
                  /*
                  TextFormField(
                    initialValue: relay0,
                    decoration: InputDecoration(
                      labelText: translate('Relay Server'),
                    ),
                    validator: validate,
                    onSaved: (String value) {
                      relay = value.trim();
                    },
                  ),
                  */
                  TextFormField(
                    initialValue: key0,
                    decoration: InputDecoration(
                      labelText: 'Key',
                    ),
                    validator: null,
                    onSaved: (String? value) {
                      if (value != null) key = value.trim();
                    },
                  ),
                ])),
            [
              TextButton(
                style: flatButtonStyle,
                onPressed: () {
                  Navigator.pop(context);
                },
                child: Text(translate('Cancel')),
              ),
              TextButton(
                style: flatButtonStyle,
                onPressed: () {
                  if (formKey.currentState != null && formKey.currentState!.validate()) {
                    formKey.currentState!.save();
                    if (id != id0)
                      FFI.setByName('option',
                          '{"name": "custom-rendezvous-server", "value": "$id"}');
                    if (relay != relay0)
                      FFI.setByName('option',
                          '{"name": "relay-server", "value": "$relay"}');
                    if (key != key0)
                      FFI.setByName(
                          'option', '{"name": "key", "value": "$key"}');
                    Navigator.pop(context);
                  }
                },
                child: Text(translate('OK')),
              ),
            ],
          ));
}

Future<Null> showAbout(BuildContext context) async {
  var version = await FFI.getVersion();
  showAlertDialog(
      context,
      (setState) => Tuple3(
          SizedBox.shrink(), // TODO test old:null
          Wrap(direction: Axis.vertical, spacing: 12, children: [
            Text('Version: $version'),
            InkWell(
                onTap: () async {
                  const url = 'https://rustdesk.com/';
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
          []),
      () async => true,
      true);
}

String? validate(value) {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = FFI.getByName('test_if_valid_server', value);
  return res.isEmpty ? null : res;
}
