import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'common.dart';

class HomePage extends StatefulWidget {
  HomePage({Key key, this.title}) : super(key: key);

  final String title;

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  final idController = TextEditingController();
  FfiModel ffi;

  @override
  Widget build(BuildContext context) {
    ffi = Provider.of<FfiModel>(context);

    // This method is rerun every time setState is called
    return Scaffold(
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
                Expanded(child: Container())
              ]),
          color: MyTheme.grayBg,
          padding: const EdgeInsets.fromLTRB(16.0, 0.0, 16.0, 0.0),
        ));
  }

  void onConnect() {
    ffi.connect(idController.text);
  }

  Widget getSearchBarUI() {
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
                    child: TextFormField(
                      style: TextStyle(
                        fontFamily: 'WorkSans',
                        fontWeight: FontWeight.bold,
                        fontSize: 30,
                        color: Color(0xFF00B6F0),
                      ),
                      keyboardType: TextInputType.text,
                      decoration: InputDecoration(
                        labelText: 'Remote ID',
                        border: InputBorder.none,
                        helperStyle: TextStyle(
                          fontWeight: FontWeight.bold,
                          fontSize: 16,
                          color: HexColor('#B9BABC'),
                        ),
                        labelStyle: TextStyle(
                          fontWeight: FontWeight.w600,
                          fontSize: 16,
                          letterSpacing: 0.2,
                          color: HexColor('#B9BABC'),
                        ),
                      ),
                      autofocus: false,
                      controller: idController,
                    ),
                  ),
                ),
                SizedBox(
                  width: 60,
                  height: 60,
                  child: IconButton(
                    icon: Icon(Icons.arrow_forward,
                        color: HexColor('#B9BABC'), size: 45),
                    onPressed: onConnect,
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
    idController.dispose();
    super.dispose();
  }
}
