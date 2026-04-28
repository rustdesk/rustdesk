import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../common.dart';
import 'screens/connect_screen.dart';
import 'screens/session_list_screen.dart';
import 'theme/app_theme.dart';
import 'theme/tokens.dart';


class AppRoot extends StatelessWidget {
  const AppRoot({super.key});

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: gFFI.ffiModel),
        ChangeNotifierProvider.value(value: gFFI.imageModel),
        ChangeNotifierProvider.value(value: gFFI.cursorModel),
        ChangeNotifierProvider.value(value: gFFI.canvasModel),
        ChangeNotifierProvider.value(value: gFFI.peerTabModel),
      ],
      child: MaterialApp(
        title: 'Tabby',
        theme: AppTheme.dark,
        navigatorKey: globalKey,
        home: const _Shell(),
      ),
    );
  }
}

class _Shell extends StatefulWidget {
  const _Shell();

  @override
  State<_Shell> createState() => _ShellState();
}

class _ShellState extends State<_Shell> {
  int _index = 0;

  static const _screens = [
    ConnectScreen(),
    SessionListScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppTokens.colorBgBase,
      body: IndexedStack(index: _index, children: _screens),
      bottomNavigationBar: NavigationBar(
        backgroundColor: AppTokens.colorBgSurface,
        indicatorColor: AppTokens.colorPrimary.withValues(alpha: 0.2),
        selectedIndex: _index,
        onDestinationSelected: (i) => setState(() => _index = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.wifi_tethering_outlined),
            selectedIcon: Icon(Icons.wifi_tethering),
            label: 'Connect',
          ),
          NavigationDestination(
            icon: Icon(Icons.devices_outlined),
            selectedIcon: Icon(Icons.devices),
            label: 'Sessions',
          ),
        ],
      ),
    );
  }
}
