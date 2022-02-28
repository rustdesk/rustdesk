import 'package:flutter/material.dart';
import 'package:flutter_hbb/pages/chat_page.dart';
import 'package:flutter_hbb/pages/server_page.dart';
import 'package:flutter_hbb/pages/settings_page.dart';
import '../common.dart';
import 'connection_page.dart';

abstract class PageShape extends Widget {
  final String title = "";
  final Icon icon = Icon(null);
  final List<Widget> appBarActions = [];
}

class HomePage extends StatefulWidget {
  HomePage({Key? key}) : super(key: key);

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  var _selectedIndex = 0;
  final List<PageShape> _pages = [
    ConnectionPage(),
    ChatPage(),
    ServerPage(),
    SettingsPage()
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: MyTheme.grayBg,
      appBar: AppBar(
        centerTitle: true,
        title: Text("RustDesk"),
        actions: _pages.elementAt(_selectedIndex).appBarActions,
      ),
      bottomNavigationBar: BottomNavigationBar(
        items: _pages
            .map((page) =>
                BottomNavigationBarItem(icon: page.icon, label: page.title))
            .toList(),
        currentIndex: _selectedIndex,
        type: BottomNavigationBarType.fixed,
        selectedItemColor: MyTheme.accent,
        unselectedItemColor: MyTheme.darkGray,
        onTap: (index) => setState(() {
          _selectedIndex = index;
        }),
      ),
      body: _pages.elementAt(_selectedIndex),
    );
  }
}
