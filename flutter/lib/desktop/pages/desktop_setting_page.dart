import 'package:flutter/cupertino.dart';

class DesktopSettingPage extends StatefulWidget {
  DesktopSettingPage({Key? key}) : super(key: key);

  @override
  State<DesktopSettingPage> createState() => _DesktopSettingPageState();
}

class _DesktopSettingPageState extends State<DesktopSettingPage> {
  @override
  Widget build(BuildContext context) {
    return Text("Settings");
  }
}
