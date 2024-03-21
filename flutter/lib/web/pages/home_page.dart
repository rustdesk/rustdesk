import 'package:flutter/material.dart';

import 'connection_page.dart';

class WebHomePage extends StatelessWidget {
  final connectionPage = ConnectionPage();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      // backgroundColor: MyTheme.grayBg,
      appBar: AppBar(
        centerTitle: true,
        title: Text("RustDesk (Beta)"),
        actions: connectionPage.appBarActions,
      ),
      body: connectionPage,
    );
  }
}
