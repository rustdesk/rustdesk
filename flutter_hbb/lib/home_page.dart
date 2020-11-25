import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
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

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    if (_idController.text.isEmpty) _idController.text = FFI.getId();
    // This method is rerun every time setState is called
    return Scaffold(
        backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          title: Text(widget.title),
        ),
        body: Container(
          child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              mainAxisSize: MainAxisSize.max,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
                getSearchBarUI(),
                getPeers(),
                Expanded(child: Container())
              ]),
          padding: const EdgeInsets.fromLTRB(16.0, 0.0, 16.0, 0.0),
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
      padding: const EdgeInsets.only(top: 8.0),
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
                      style: TextStyle(
                        fontFamily: 'WorkSans',
                        fontWeight: FontWeight.bold,
                        fontSize: 30,
                        color: Color(0xFF00B6F0),
                      ),
                      // keyboardType: TextInputType.number,
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
    return Image.asset('assets/$platform.png', width: 36, height: 36);
  }

  Widget getPeers() {
    final cards = <Widget>[];
    var peers = FFI.peers();
    peers.forEach((p) {
      cards.add(Card(
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
              ))));
    });
    return Wrap(children: cards);
  }
}
