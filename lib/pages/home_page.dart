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
  final ScrollController? scrollController = null;
}

class HomePage extends StatefulWidget {
  HomePage({Key? key}) : super(key: key);

  @override
  _HomePageState createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  var _selectedIndex = 0;
  final List<PageShape> _pages = [];

  @override
  void initState() {
    super.initState();
    _pages.add(ConnectionPage());
    if (isAndroid) {
      _pages.addAll([chatPage, ServerPage()]);
    }
    _pages.add(SettingsPage());
  }

  @override
  Widget build(BuildContext context) {
    return WillPopScope(
        onWillPop: () async {
          if (_selectedIndex != 0) {
            setState(() {
              _selectedIndex = 0;
            });
          } else {
            return true;
          }
          return false;
        },
        child: Scaffold(
          backgroundColor: MyTheme.grayBg,
          appBar: AppBar(
            centerTitle: true,
            title: Text("RustDesk"),
            actions: _pages.elementAt(_selectedIndex).appBarActions,
          ),
          bottomNavigationBar: BottomNavigationBar(
            key: navigationBarKey,
            items: _pages
                .map((page) =>
                    BottomNavigationBarItem(icon: page.icon, label: page.title))
                .toList(),
            currentIndex: _selectedIndex,
            type: BottomNavigationBarType.fixed,
            selectedItemColor: MyTheme.accent,
            unselectedItemColor: MyTheme.darkGray,
            onTap: (index) => setState(() {
              // close chat overlay when go chat page
              if (index == 1 && _selectedIndex != index) {
                hideChatIconOverlay();
                hideChatWindowOverlay();
              }
              _selectedIndex = index;
            }),
          ),
          body: Listener(
              onPointerMove: (evt) {
                final page = _pages.elementAt(_selectedIndex);

                /// Flutter can't not catch PointerMoveEvent when size is 1
                /// This will happen in Android AccessibilityService Input
                /// android can't init dispatching size yet ,see: https://stackoverflow.com/questions/59960451/android-accessibility-dispatchgesture-is-it-possible-to-specify-pressure-for-a
                /// use this temporary solution until flutter or android fixes the bug
                if (evt.size == 1 && page.scrollController != null) {
                  final offset = page.scrollController!.offset.toDouble();
                  page.scrollController!.jumpTo(offset - evt.delta.dy);
                }
              },
              child: _pages.elementAt(_selectedIndex)),
        ));
  }
}

class WebHomePage extends StatelessWidget {
  final connectionPage = ConnectionPage();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: MyTheme.grayBg,
      appBar: AppBar(
        centerTitle: true,
        title: Text("RustDesk" + (isWeb ? " (Beta) " : "")),
        actions: connectionPage.appBarActions,
      ),
      body: connectionPage,
    );
  }
}
