import 'package:flutter/material.dart';

class DesktopHomePage extends StatefulWidget {
  DesktopHomePage({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _DesktopHomePageState();
}

class _DesktopHomePageState extends State<DesktopHomePage> {
  @override
  Widget build(BuildContext context) {
    return Text("Hello Desktop");
  }
}
